//! Encrypted vault domain and persistence boundary.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    ptr::NonNull,
    slice,
    sync::OnceLock,
};

use libsodium_sys as sodium;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::platform::{NativePlatformSecurity, PlatformSecurity};

#[cfg(feature = "fuzzing")]
pub mod fuzzing;

const MAGIC: [u8; 8] = *b"SNOTES\0\0";
const FORMAT_VERSION: u16 = 1;
const SCHEMA_VERSION: u16 = 1;
const DATA_KEY_LEN: usize = sodium::crypto_aead_xchacha20poly1305_ietf_KEYBYTES as usize;
const NONCE_LEN: usize = sodium::crypto_aead_xchacha20poly1305_ietf_NPUBBYTES as usize;
const TAG_LEN: usize = sodium::crypto_aead_xchacha20poly1305_ietf_ABYTES as usize;
const SALT_LEN: usize = sodium::crypto_pwhash_SALTBYTES as usize;
const WRAPPED_KEY_LEN: usize = DATA_KEY_LEN + TAG_LEN;
const WRAP_AAD_LEN: usize = 88;
#[cfg(test)]
pub(crate) const WRAPPED_KEY_OFFSET: usize = 88;
pub(crate) const PAYLOAD_LENGTH_OFFSET: usize = 160;
pub(crate) const HEADER_LEN: usize = 168;
pub const PAYLOAD_LIMIT: usize = 100 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Note {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub modified_at: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Vault {
    pub schema_version: u16,
    pub notes: Vec<Note>,
}

impl Default for Vault {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VaultError {
    #[error("unsupported vault version: {0}")]
    UnsupportedVersion(u16),
    #[error("vault payload exceeds the 100 MB limit")]
    PayloadTooLarge,
    #[error("incorrect password or damaged key data")]
    InvalidPasswordOrKey,
    #[error("vault integrity check failed")]
    IntegrityViolation,
    #[error("malformed vault container")]
    MalformedContainer,
    #[error("password must contain at least 8 characters")]
    PasswordTooShort,
    #[error("secure memory could not be locked; operation aborted")]
    MemoryLockFailed,
    #[error("cryptographic subsystem is unavailable")]
    CryptoUnavailable,
    #[error("vault file was changed or removed by another process")]
    FileConflict,
    #[error("vault file could not be read")]
    ReadFailed,
    #[error("vault file could not be written")]
    WriteFailed,
}

pub fn validate_password(password: &str) -> Result<(), VaultError> {
    if password.chars().count() < 8 {
        return Err(VaultError::PasswordTooShort);
    }
    Ok(())
}

/// A guarded allocation that is explicitly locked and wiped before release.
pub struct SecretBytes {
    ptr: NonNull<u8>,
    len: usize,
}

impl SecretBytes {
    fn allocate(len: usize) -> Result<Self, VaultError> {
        ensure_sodium()?;
        let raw = unsafe { sodium::sodium_malloc(len) }.cast::<u8>();
        let Some(ptr) = NonNull::new(raw) else {
            return Err(VaultError::MemoryLockFailed);
        };
        let bytes = unsafe { slice::from_raw_parts_mut(raw, len) };
        if NativePlatformSecurity.lock_memory(bytes).is_err() {
            unsafe { sodium::sodium_free(raw.cast()) };
            return Err(VaultError::MemoryLockFailed);
        }
        Ok(Self { ptr, len })
    }

    fn random(len: usize) -> Result<Self, VaultError> {
        let secret = Self::allocate(len)?;
        unsafe { sodium::randombytes_buf(secret.ptr.as_ptr().cast(), len) };
        Ok(secret)
    }

    fn from_slice(bytes: &[u8]) -> Result<Self, VaultError> {
        let mut secret = Self::allocate(bytes.len())?;
        secret.expose_mut().copy_from_slice(bytes);
        Ok(secret)
    }

    pub(crate) fn expose(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    fn expose_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        unsafe {
            sodium::sodium_memzero(self.ptr.as_ptr().cast(), self.len);
            NativePlatformSecurity
                .unlock_memory(slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len));
            sodium::sodium_free(self.ptr.as_ptr().cast());
        }
    }
}

unsafe impl Send for SecretBytes {}

#[derive(Clone, Copy)]
pub(crate) struct KdfProfile {
    ops_limit: u64,
    mem_limit: u64,
}

impl KdfProfile {
    pub(crate) const fn production() -> Self {
        Self {
            ops_limit: 3,
            mem_limit: 256 * 1024 * 1024,
        }
    }

    #[cfg(test)]
    pub(crate) const fn testing() -> Self {
        Self {
            ops_limit: 1,
            mem_limit: 8 * 1024 * 1024,
        }
    }
}

pub(crate) struct VaultCodec {
    profile: KdfProfile,
}

impl VaultCodec {
    pub(crate) const fn new(profile: KdfProfile) -> Self {
        Self { profile }
    }

    pub(crate) fn encrypt_new(
        &self,
        vault: &Vault,
        password: &str,
    ) -> Result<(Vec<u8>, SecretBytes), VaultError> {
        validate_password(password)?;
        validate_schema(vault)?;
        let data_key = SecretBytes::random(DATA_KEY_LEN)?;
        let container = self.encrypt(vault, password, &data_key, random_array()?)?;
        Ok((container, data_key))
    }

    pub(crate) fn decrypt(
        &self,
        container: &[u8],
        password: &str,
    ) -> Result<(Vault, SecretBytes), VaultError> {
        let header = Header::parse(container, self.profile)?;
        let kek = derive_key(password, &header.salt, self.profile)?;
        let key_bytes = aead_decrypt(
            &header.wrapped_key,
            kek.expose(),
            &header.wrap_nonce,
            &container[..WRAP_AAD_LEN],
        )
        .map_err(|_| VaultError::InvalidPasswordOrKey)?;
        let data_key = SecretBytes::from_slice(&key_bytes)?;
        wipe_vec(key_bytes);

        let ciphertext = &container[HEADER_LEN..];
        let plaintext = aead_decrypt(
            ciphertext,
            data_key.expose(),
            &header.payload_nonce,
            &container[..HEADER_LEN],
        )
        .map_err(|_| VaultError::IntegrityViolation)?;
        if plaintext.len() > PAYLOAD_LIMIT {
            wipe_vec(plaintext);
            return Err(VaultError::PayloadTooLarge);
        }
        let decoded =
            serde_json::from_slice::<Vault>(&plaintext).map_err(|_| VaultError::IntegrityViolation);
        wipe_vec(plaintext);
        let vault = decoded?;
        validate_schema(&vault)?;
        Ok((vault, data_key))
    }

    fn encrypt_updated(
        &self,
        existing: &[u8],
        vault: &Vault,
        data_key: &SecretBytes,
    ) -> Result<Vec<u8>, VaultError> {
        validate_schema(vault)?;
        let mut header = Header::parse(existing, self.profile)?;
        let plaintext = serde_json::to_vec(vault).map_err(|_| VaultError::IntegrityViolation)?;
        if plaintext.len() > PAYLOAD_LIMIT {
            wipe_vec(plaintext);
            return Err(VaultError::PayloadTooLarge);
        }
        header.payload_nonce = random_array()?;
        header.payload_len = (plaintext.len() + TAG_LEN) as u64;
        let serialized = header.serialize();
        let encrypted = aead_encrypt(
            &plaintext,
            data_key.expose(),
            &header.payload_nonce,
            &serialized,
        );
        wipe_vec(plaintext);
        let encrypted = encrypted?;
        let mut container = Vec::with_capacity(HEADER_LEN + encrypted.len());
        container.extend_from_slice(&serialized);
        container.extend_from_slice(&encrypted);
        Ok(container)
    }

    fn encrypt_rewrapped(
        &self,
        existing: &[u8],
        vault: &Vault,
        data_key: &SecretBytes,
        new_password: &str,
    ) -> Result<Vec<u8>, VaultError> {
        validate_password(new_password)?;
        let old_header = Header::parse(existing, self.profile)?;
        self.encrypt(vault, new_password, data_key, old_header.vault_id)
    }

    fn encrypt(
        &self,
        vault: &Vault,
        password: &str,
        data_key: &SecretBytes,
        vault_id: [u8; 16],
    ) -> Result<Vec<u8>, VaultError> {
        validate_schema(vault)?;
        let plaintext = serde_json::to_vec(vault).map_err(|_| VaultError::IntegrityViolation)?;
        if plaintext.len() > PAYLOAD_LIMIT {
            wipe_vec(plaintext);
            return Err(VaultError::PayloadTooLarge);
        }

        let salt = random_array()?;
        let wrap_nonce = random_array()?;
        let payload_nonce = random_array()?;
        let kek = derive_key(password, &salt, self.profile)?;

        let mut header = Header {
            vault_id,
            salt,
            ops_limit: self.profile.ops_limit,
            mem_limit: self.profile.mem_limit,
            wrap_nonce,
            wrapped_key: [0; WRAPPED_KEY_LEN],
            payload_nonce,
            payload_len: (plaintext.len() + TAG_LEN) as u64,
        };
        let partial = header.serialize();
        let wrapped = aead_encrypt(
            data_key.expose(),
            kek.expose(),
            &header.wrap_nonce,
            &partial[..WRAP_AAD_LEN],
        )?;
        header.wrapped_key.copy_from_slice(&wrapped);
        let serialized = header.serialize();
        let encrypted = aead_encrypt(
            &plaintext,
            data_key.expose(),
            &header.payload_nonce,
            &serialized,
        );
        wipe_vec(plaintext);
        let encrypted = encrypted?;

        let mut container = Vec::with_capacity(HEADER_LEN + encrypted.len());
        container.extend_from_slice(&serialized);
        container.extend_from_slice(&encrypted);
        Ok(container)
    }
}

pub struct VaultSession {
    pub path: PathBuf,
    pub vault: Vault,
    pub data_key: SecretBytes,
    pub fingerprint: [u8; 32],
    pub dirty: bool,
}

pub struct VaultService {
    codec: VaultCodec,
}

impl Default for VaultService {
    fn default() -> Self {
        Self {
            codec: VaultCodec::new(KdfProfile::production()),
        }
    }
}

impl VaultService {
    #[cfg(test)]
    fn testing() -> Self {
        Self {
            codec: VaultCodec::new(KdfProfile::testing()),
        }
    }

    pub fn create(
        &self,
        path: impl AsRef<Path>,
        password: &str,
    ) -> Result<VaultSession, VaultError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(VaultError::FileConflict);
        }
        let vault = Vault::default();
        let (container, data_key) = self.codec.encrypt_new(&vault, password)?;
        atomic_write(path, &container, None, false)?;
        Ok(VaultSession {
            path: path.to_path_buf(),
            vault,
            data_key,
            fingerprint: fingerprint(&container)?,
            dirty: false,
        })
    }

    pub fn open(&self, path: impl AsRef<Path>, password: &str) -> Result<VaultSession, VaultError> {
        let path = path.as_ref();
        let container = read_container(path)?;
        let fingerprint = fingerprint(&container)?;
        let (vault, data_key) = self.codec.decrypt(&container, password)?;
        Ok(VaultSession {
            path: path.to_path_buf(),
            vault,
            data_key,
            fingerprint,
            dirty: false,
        })
    }

    pub fn save(&self, session: &mut VaultSession) -> Result<(), VaultError> {
        self.save_inner(session, None)
    }

    pub fn save_after_reauthentication(
        &self,
        session: &mut VaultSession,
        password: &str,
    ) -> Result<(), VaultError> {
        let existing = self.current_container(session)?;
        let (_, authenticated_key) = self.codec.decrypt(&existing, password)?;
        if !constant_time_equal(authenticated_key.expose(), session.data_key.expose()) {
            return Err(VaultError::InvalidPasswordOrKey);
        }
        self.save_inner(session, None)
    }

    pub fn change_password(
        &self,
        session: &mut VaultSession,
        new_password: &str,
    ) -> Result<(), VaultError> {
        let existing = self.current_container(session)?;
        let container = self.codec.encrypt_rewrapped(
            &existing,
            &session.vault,
            &session.data_key,
            new_password,
        )?;
        atomic_write(&session.path, &container, None, true)?;
        session.fingerprint = fingerprint(&container)?;
        session.dirty = false;
        Ok(())
    }

    pub fn save_copy(
        &self,
        session: &mut VaultSession,
        new_path: impl AsRef<Path>,
        password: &str,
    ) -> Result<(), VaultError> {
        let new_path = new_path.as_ref();
        if new_path.exists() {
            return Err(VaultError::FileConflict);
        }
        let container =
            self.codec
                .encrypt(&session.vault, password, &session.data_key, random_array()?)?;
        atomic_write(new_path, &container, None, false)?;
        session.path = new_path.to_path_buf();
        session.fingerprint = fingerprint(&container)?;
        session.dirty = false;
        Ok(())
    }

    fn save_inner(
        &self,
        session: &mut VaultSession,
        fault: Option<FaultPoint>,
    ) -> Result<(), VaultError> {
        let existing = self.current_container(session)?;
        let container = self
            .codec
            .encrypt_updated(&existing, &session.vault, &session.data_key)?;
        atomic_write(&session.path, &container, fault, true)?;
        session.fingerprint = fingerprint(&container)?;
        session.dirty = false;
        Ok(())
    }

    fn current_container(&self, session: &VaultSession) -> Result<Vec<u8>, VaultError> {
        let current = read_container(&session.path).map_err(|error| match error {
            VaultError::PayloadTooLarge => VaultError::PayloadTooLarge,
            _ => VaultError::FileConflict,
        })?;
        if fingerprint(&current)? != session.fingerprint {
            return Err(VaultError::FileConflict);
        }
        Ok(current)
    }

    #[cfg(test)]
    fn save_with_fault(
        &self,
        session: &mut VaultSession,
        point: FaultPoint,
    ) -> Result<(), VaultError> {
        self.save_inner(session, Some(point))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FaultPoint {
    AfterCreate,
    AfterWrite,
    AfterSync,
    BeforeReplace,
    AfterReplace,
}

#[cfg(test)]
impl FaultPoint {
    const ALL: [Self; 5] = [
        Self::AfterCreate,
        Self::AfterWrite,
        Self::AfterSync,
        Self::BeforeReplace,
        Self::AfterReplace,
    ];
}

fn read_container(path: &Path) -> Result<Vec<u8>, VaultError> {
    let metadata = fs::metadata(path).map_err(|_| VaultError::ReadFailed)?;
    let max_size = HEADER_LEN as u64 + PAYLOAD_LIMIT as u64 + TAG_LEN as u64;
    if metadata.len() > max_size {
        return Err(VaultError::PayloadTooLarge);
    }
    fs::read(path).map_err(|_| VaultError::ReadFailed)
}

fn fingerprint(bytes: &[u8]) -> Result<[u8; 32], VaultError> {
    ensure_sodium()?;
    let mut digest = [0_u8; 32];
    let result = unsafe {
        sodium::crypto_generichash(
            digest.as_mut_ptr(),
            digest.len(),
            bytes.as_ptr(),
            bytes.len() as u64,
            std::ptr::null(),
            0,
        )
    };
    if result != 0 {
        return Err(VaultError::CryptoUnavailable);
    }
    Ok(digest)
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && unsafe {
            sodium::sodium_memcmp(left.as_ptr().cast(), right.as_ptr().cast(), left.len()) == 0
        }
}

fn atomic_write(
    path: &Path,
    ciphertext: &[u8],
    fault: Option<FaultPoint>,
    replace_existing: bool,
) -> Result<(), VaultError> {
    let parent = path.parent().ok_or(VaultError::WriteFailed)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(VaultError::WriteFailed)?;
    let random: [u8; 8] = random_array()?;
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let temporary = parent.join(format!(".{file_name}.{suffix}.tmp"));

    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| VaultError::WriteFailed)?;
        inject_fault(fault, FaultPoint::AfterCreate)?;
        file.write_all(ciphertext)
            .map_err(|_| VaultError::WriteFailed)?;
        inject_fault(fault, FaultPoint::AfterWrite)?;
        file.sync_all().map_err(|_| VaultError::WriteFailed)?;
        inject_fault(fault, FaultPoint::AfterSync)?;
        drop(file);
        inject_fault(fault, FaultPoint::BeforeReplace)?;
        replace_file(&temporary, path, replace_existing)?;
        inject_fault(fault, FaultPoint::AfterReplace)?;
        sync_parent(parent)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn inject_fault(actual: Option<FaultPoint>, current: FaultPoint) -> Result<(), VaultError> {
    if actual == Some(current) {
        Err(VaultError::WriteFailed)
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(
    temporary: &Path,
    destination: &Path,
    replace_existing: bool,
) -> Result<(), VaultError> {
    if replace_existing {
        fs::rename(temporary, destination).map_err(|_| VaultError::WriteFailed)
    } else {
        fs::hard_link(temporary, destination).map_err(|_| VaultError::WriteFailed)?;
        fs::remove_file(temporary).map_err(|_| VaultError::WriteFailed)
    }
}

#[cfg(windows)]
fn replace_file(
    temporary: &Path,
    destination: &Path,
    replace_existing: bool,
) -> Result<(), VaultError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    let temporary: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let flags = if replace_existing {
        MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
    } else {
        MOVEFILE_WRITE_THROUGH
    };
    unsafe {
        MoveFileExW(
            PCWSTR(temporary.as_ptr()),
            PCWSTR(destination.as_ptr()),
            flags,
        )
    }
    .map_err(|_| VaultError::WriteFailed)
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<(), VaultError> {
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| VaultError::WriteFailed)
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<(), VaultError> {
    Ok(())
}

#[derive(Clone)]
struct Header {
    vault_id: [u8; 16],
    salt: [u8; SALT_LEN],
    ops_limit: u64,
    mem_limit: u64,
    wrap_nonce: [u8; NONCE_LEN],
    wrapped_key: [u8; WRAPPED_KEY_LEN],
    payload_nonce: [u8; NONCE_LEN],
    payload_len: u64,
}

impl Header {
    fn parse(container: &[u8], expected: KdfProfile) -> Result<Self, VaultError> {
        if container.len() < HEADER_LEN {
            return Err(VaultError::MalformedContainer);
        }
        if container[..8] != MAGIC {
            return Err(VaultError::MalformedContainer);
        }
        let version = read_u16(container, 8);
        if version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        if read_u16(container, 10) != 0 {
            return Err(VaultError::MalformedContainer);
        }

        let ops_limit = read_u64(container, 44);
        let mem_limit = read_u64(container, 52);
        let algorithm = u32::from_le_bytes(container[60..64].try_into().unwrap());
        if ops_limit != expected.ops_limit
            || mem_limit != expected.mem_limit
            || algorithm != sodium::crypto_pwhash_ALG_ARGON2ID13
        {
            return Err(VaultError::MalformedContainer);
        }
        let payload_len = read_u64(container, PAYLOAD_LENGTH_OFFSET);
        validate_ciphertext_length(payload_len, container.len())?;

        Ok(Self {
            vault_id: container[12..28].try_into().unwrap(),
            salt: container[28..44].try_into().unwrap(),
            ops_limit,
            mem_limit,
            wrap_nonce: container[64..88].try_into().unwrap(),
            wrapped_key: container[88..136].try_into().unwrap(),
            payload_nonce: container[136..160].try_into().unwrap(),
            payload_len,
        })
    }

    fn serialize(&self) -> [u8; HEADER_LEN] {
        let mut bytes = [0_u8; HEADER_LEN];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes[12..28].copy_from_slice(&self.vault_id);
        bytes[28..44].copy_from_slice(&self.salt);
        bytes[44..52].copy_from_slice(&self.ops_limit.to_le_bytes());
        bytes[52..60].copy_from_slice(&self.mem_limit.to_le_bytes());
        bytes[60..64].copy_from_slice(&sodium::crypto_pwhash_ALG_ARGON2ID13.to_le_bytes());
        bytes[64..88].copy_from_slice(&self.wrap_nonce);
        bytes[88..136].copy_from_slice(&self.wrapped_key);
        bytes[136..160].copy_from_slice(&self.payload_nonce);
        bytes[160..168].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes
    }
}

fn validate_ciphertext_length(payload_len: u64, container_len: usize) -> Result<(), VaultError> {
    let max_ciphertext = PAYLOAD_LIMIT as u64 + TAG_LEN as u64;
    if payload_len > max_ciphertext {
        return Err(VaultError::PayloadTooLarge);
    }
    if payload_len < TAG_LEN as u64
        || usize::try_from(payload_len)
            .ok()
            .and_then(|len| HEADER_LEN.checked_add(len))
            != Some(container_len)
    {
        return Err(VaultError::MalformedContainer);
    }
    Ok(())
}

fn validate_schema(vault: &Vault) -> Result<(), VaultError> {
    if vault.schema_version != SCHEMA_VERSION {
        return Err(VaultError::UnsupportedVersion(vault.schema_version));
    }
    Ok(())
}

fn ensure_sodium() -> Result<(), VaultError> {
    static INITIALIZED: OnceLock<bool> = OnceLock::new();
    if *INITIALIZED.get_or_init(|| unsafe { sodium::sodium_init() >= 0 }) {
        Ok(())
    } else {
        Err(VaultError::CryptoUnavailable)
    }
}

fn derive_key(
    password: &str,
    salt: &[u8; SALT_LEN],
    profile: KdfProfile,
) -> Result<SecretBytes, VaultError> {
    ensure_sodium()?;
    let mut key = SecretBytes::allocate(DATA_KEY_LEN)?;
    let result = unsafe {
        sodium::crypto_pwhash(
            key.expose_mut().as_mut_ptr(),
            DATA_KEY_LEN as u64,
            password.as_ptr().cast(),
            password.len() as u64,
            salt.as_ptr(),
            profile.ops_limit,
            profile.mem_limit as usize,
            sodium::crypto_pwhash_ALG_ARGON2ID13 as i32,
        )
    };
    if result != 0 {
        return Err(VaultError::CryptoUnavailable);
    }
    Ok(key)
}

fn aead_encrypt(
    message: &[u8],
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
) -> Result<Vec<u8>, VaultError> {
    ensure_sodium()?;
    if key.len() != DATA_KEY_LEN {
        return Err(VaultError::CryptoUnavailable);
    }
    let mut output = vec![0_u8; message.len() + TAG_LEN];
    let mut output_len = 0_u64;
    let result = unsafe {
        sodium::crypto_aead_xchacha20poly1305_ietf_encrypt(
            output.as_mut_ptr(),
            &mut output_len,
            message.as_ptr(),
            message.len() as u64,
            aad.as_ptr(),
            aad.len() as u64,
            std::ptr::null(),
            nonce.as_ptr(),
            key.as_ptr(),
        )
    };
    if result != 0 || output_len as usize != output.len() {
        wipe_vec(output);
        return Err(VaultError::CryptoUnavailable);
    }
    Ok(output)
}

fn aead_decrypt(
    ciphertext: &[u8],
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
) -> Result<Vec<u8>, ()> {
    if ensure_sodium().is_err() || key.len() != DATA_KEY_LEN || ciphertext.len() < TAG_LEN {
        return Err(());
    }
    let mut output = vec![0_u8; ciphertext.len() - TAG_LEN];
    let mut output_len = 0_u64;
    let result = unsafe {
        sodium::crypto_aead_xchacha20poly1305_ietf_decrypt(
            output.as_mut_ptr(),
            &mut output_len,
            std::ptr::null_mut(),
            ciphertext.as_ptr(),
            ciphertext.len() as u64,
            aad.as_ptr(),
            aad.len() as u64,
            nonce.as_ptr(),
            key.as_ptr(),
        )
    };
    if result != 0 || output_len as usize != output.len() {
        wipe_vec(output);
        return Err(());
    }
    Ok(output)
}

fn random_array<const N: usize>() -> Result<[u8; N], VaultError> {
    ensure_sodium()?;
    let mut bytes = [0_u8; N];
    unsafe { sodium::randombytes_buf(bytes.as_mut_ptr().cast(), N) };
    Ok(bytes)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn wipe_vec(mut bytes: Vec<u8>) {
    if !bytes.is_empty() {
        unsafe { sodium::sodium_memzero(bytes.as_mut_ptr().cast(), bytes.len()) };
    }
}

#[cfg(test)]
mod tests;
