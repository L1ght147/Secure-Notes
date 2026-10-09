use super::{
    FaultPoint, HEADER_LEN, KdfProfile, Note, PAYLOAD_LENGTH_OFFSET, PAYLOAD_LIMIT, Vault,
    VaultCodec, VaultError, VaultService, WRAPPED_KEY_OFFSET, atomic_write,
    validate_ciphertext_length, validate_password,
};
use std::ffi::CStr;
use std::fs;
use uuid::Uuid;

fn sample_vault() -> Vault {
    Vault {
        schema_version: 1,
        notes: vec![Note {
            id: Uuid::from_u128(0x1234),
            title: "Заметка 🔐".into(),
            body: "こんにちは — confidential".into(),
            modified_at: 1_700_000_000,
        }],
    }
}

#[test]
fn encrypted_container_round_trips_unicode_without_plaintext() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let original = sample_vault();

    let (container, data_key) = codec.encrypt_new(&original, "correct horse").unwrap();
    let (opened, _key) = codec.decrypt(&container, "correct horse").unwrap();

    assert_eq!(opened, original);
    assert!(
        !container
            .windows("confidential".len())
            .any(|window| window == b"confidential")
    );
    drop(data_key);
}

#[test]
fn linked_libsodium_version_is_pinned() {
    super::ensure_sodium().unwrap();
    let version = unsafe { CStr::from_ptr(libsodium_sys::sodium_version_string()) };
    assert_eq!(version.to_bytes(), b"1.0.22");
}

#[test]
fn production_service_uses_argon2id_256_mib_and_three_passes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("production.snotes");
    let service = VaultService::default();
    service.create(&path, "correct horse").unwrap();
    let container = fs::read(&path).unwrap();

    assert_eq!(u64::from_le_bytes(container[44..52].try_into().unwrap()), 3);
    assert_eq!(
        u64::from_le_bytes(container[52..60].try_into().unwrap()),
        256 * 1024 * 1024
    );
    assert_eq!(u32::from_le_bytes(container[60..64].try_into().unwrap()), 2);
    service.open(&path, "correct horse").unwrap();
}

#[test]
fn wrong_password_and_damaged_wrapped_key_are_indistinguishable() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (container, _key) = codec.encrypt_new(&sample_vault(), "correct horse").unwrap();

    assert!(matches!(
        codec.decrypt(&container, "wrong password"),
        Err(VaultError::InvalidPasswordOrKey)
    ));

    let mut damaged = container;
    damaged[WRAPPED_KEY_OFFSET] ^= 0x80;
    assert!(matches!(
        codec.decrypt(&damaged, "correct horse"),
        Err(VaultError::InvalidPasswordOrKey)
    ));
}

#[test]
fn payload_tampering_is_an_integrity_error() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (mut container, _key) = codec.encrypt_new(&sample_vault(), "correct horse").unwrap();
    let final_byte = container.last_mut().unwrap();
    *final_byte ^= 1;

    assert!(matches!(
        codec.decrypt(&container, "correct horse"),
        Err(VaultError::IntegrityViolation)
    ));
}

#[test]
fn truncated_container_is_rejected() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (container, _key) = codec.encrypt_new(&sample_vault(), "correct horse").unwrap();

    assert!(matches!(
        codec.decrypt(&container[..HEADER_LEN - 1], "correct horse"),
        Err(VaultError::MalformedContainer)
    ));
}

#[test]
fn unknown_container_version_is_rejected_before_kdf() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (mut container, _key) = codec.encrypt_new(&sample_vault(), "correct horse").unwrap();
    container[8..10].copy_from_slice(&2_u16.to_le_bytes());

    assert!(matches!(
        codec.decrypt(&container, "correct horse"),
        Err(VaultError::UnsupportedVersion(2))
    ));
}

#[test]
fn payload_size_limit_is_checked_before_allocation() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (mut container, _key) = codec.encrypt_new(&sample_vault(), "correct horse").unwrap();
    let too_large = (PAYLOAD_LIMIT as u64 + 17).to_le_bytes();
    container[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 8].copy_from_slice(&too_large);

    assert!(matches!(
        codec.decrypt(&container, "correct horse"),
        Err(VaultError::PayloadTooLarge)
    ));
}

#[test]
fn payload_limit_accepts_exact_boundary_and_rejects_one_byte_more() {
    let maximum = PAYLOAD_LIMIT as u64 + 16;
    assert!(validate_ciphertext_length(maximum, HEADER_LEN + maximum as usize).is_ok());
    assert!(matches!(
        validate_ciphertext_length(maximum + 1, HEADER_LEN),
        Err(VaultError::PayloadTooLarge)
    ));
    assert!(matches!(
        validate_ciphertext_length(15, HEADER_LEN + 15),
        Err(VaultError::MalformedContainer)
    ));
}

#[test]
fn password_requires_eight_unicode_characters() {
    assert!(validate_password("пароль!🔐").is_ok());
    assert!(matches!(
        validate_password("1234567"),
        Err(VaultError::PasswordTooShort)
    ));
}

#[test]
fn service_creates_saves_opens_and_changes_password() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.snotes");
    let service = VaultService::testing();
    let mut session = service.create(&path, "old password").unwrap();
    let original_key: [u8; 32] = session.data_key.expose().try_into().unwrap();
    session.vault = sample_vault();
    session.dirty = true;

    service.save(&mut session).unwrap();
    service
        .change_password(&mut session, "new password")
        .unwrap();
    assert_eq!(session.data_key.expose(), original_key);

    assert!(matches!(
        service.open(&path, "old password"),
        Err(VaultError::InvalidPasswordOrKey)
    ));
    let opened = service.open(&path, "new password").unwrap();
    assert_eq!(opened.vault, sample_vault());
    assert!(!opened.dirty);
}

#[test]
fn save_refuses_to_overwrite_external_change_or_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.snotes");
    let service = VaultService::testing();
    let mut changed = service.create(&path, "correct horse").unwrap();
    changed.dirty = true;
    fs::write(&path, b"changed outside").unwrap();

    assert!(matches!(
        service.save(&mut changed),
        Err(VaultError::FileConflict)
    ));

    let deleted_path = directory.path().join("deleted.snotes");
    let mut deleted = service.create(&deleted_path, "correct horse").unwrap();
    deleted.dirty = true;
    fs::remove_file(&deleted_path).unwrap();
    assert!(matches!(
        service.save(&mut deleted),
        Err(VaultError::FileConflict)
    ));
}

#[test]
fn every_atomic_save_failure_leaves_old_or_new_valid_vault() {
    for point in FaultPoint::ALL {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.snotes");
        let service = VaultService::testing();
        let mut session = service.create(&path, "correct horse").unwrap();
        session.vault = sample_vault();
        session.dirty = true;

        assert!(matches!(
            service.save_with_fault(&mut session, point),
            Err(VaultError::WriteFailed)
        ));

        let opened = service.open(&path, "correct horse").unwrap();
        let expected = if point == FaultPoint::AfterReplace {
            sample_vault()
        } else {
            Vault::default()
        };
        assert_eq!(opened.vault, expected);
        for entry in fs::read_dir(directory.path()).unwrap() {
            let bytes = fs::read(entry.unwrap().path()).unwrap();
            assert!(
                !bytes
                    .windows("confidential".len())
                    .any(|window| window == b"confidential")
            );
        }
    }
}

#[test]
fn conflict_can_be_saved_as_an_encrypted_copy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.snotes");
    let copy = directory.path().join("copy.snotes");
    let service = VaultService::testing();
    let mut session = service.create(&path, "correct horse").unwrap();
    session.vault = sample_vault();
    session.dirty = true;
    fs::write(&path, b"changed outside").unwrap();

    service
        .save_copy(&mut session, &copy, "correct horse")
        .unwrap();

    assert_eq!(
        service.open(&copy, "correct horse").unwrap().vault,
        sample_vault()
    );
}

#[test]
fn soft_lock_retry_requires_password_before_saving_ram_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.snotes");
    let service = VaultService::testing();
    let mut session = service.create(&path, "correct horse").unwrap();
    session.vault = sample_vault();
    session.dirty = true;

    assert!(matches!(
        service.save_after_reauthentication(&mut session, "wrong password"),
        Err(VaultError::InvalidPasswordOrKey)
    ));
    assert!(session.dirty);

    service
        .save_after_reauthentication(&mut session, "correct horse")
        .unwrap();
    assert!(!session.dirty);
    assert_eq!(
        service.open(&path, "correct horse").unwrap().vault,
        sample_vault()
    );
}

#[test]
fn complete_vault_lifecycle_never_writes_known_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.snotes");
    let service = VaultService::testing();
    let mut session = service.create(&path, "correct horse").unwrap();
    session.vault.notes.push(Note {
        id: Uuid::from_u128(99),
        title: "PLAINTEXT-CANARY-TITLE".into(),
        body: "PLAINTEXT-CANARY-BODY".into(),
        modified_at: 42,
    });
    session.dirty = true;
    service.save(&mut session).unwrap();
    service
        .change_password(&mut session, "different password")
        .unwrap();

    for entry in fs::read_dir(directory.path()).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        for canary in [
            b"PLAINTEXT-CANARY-TITLE".as_slice(),
            b"PLAINTEXT-CANARY-BODY",
        ] {
            assert!(!bytes.windows(canary.len()).any(|window| window == canary));
        }
    }
}

#[test]
fn create_mode_atomic_write_never_replaces_an_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("existing.snotes");
    fs::write(&path, b"original").unwrap();

    assert!(matches!(
        atomic_write(&path, b"replacement", None, false),
        Err(VaultError::WriteFailed)
    ));
    assert_eq!(fs::read(path).unwrap(), b"original");
}

#[test]
fn encrypted_copy_rejects_short_password_without_creating_file() {
    let directory = tempfile::tempdir().unwrap();
    let service = VaultService::testing();
    let mut session = service
        .create(directory.path().join("vault.snotes"), "original password")
        .unwrap();
    let copy = directory.path().join("copy.snotes");
    assert_eq!(
        service.save_copy(&mut session, &copy, "x"),
        Err(VaultError::PasswordTooShort)
    );
    assert!(!copy.exists());
}

#[test]
fn save_refuses_while_another_writer_holds_the_vault_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.snotes");
    let service = VaultService::testing();
    let mut session = service.create(&path, "original password").unwrap();
    let before = fs::read(&path).unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.path().join(".vault.snotes.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    session.vault = sample_vault();
    session.dirty = true;
    assert_eq!(service.save(&mut session), Err(VaultError::FileConflict));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(session.dirty);
}

#[test]
fn reauthentication_tracks_password_changes_and_copies() {
    let directory = tempfile::tempdir().unwrap();
    let service = VaultService::testing();
    let mut session = service
        .create(directory.path().join("vault.snotes"), "original password")
        .unwrap();
    service
        .change_password(&mut session, "replacement password")
        .unwrap();
    assert_eq!(
        service.authenticate_session(&session, "original password"),
        Err(VaultError::InvalidPasswordOrKey)
    );
    service
        .authenticate_session(&session, "replacement password")
        .unwrap();
    service
        .save_copy_after_reauthentication(
            &mut session,
            directory.path().join("copy.snotes"),
            "replacement password",
            "copy password",
        )
        .unwrap();
    assert_eq!(
        service.authenticate_session(&session, "replacement password"),
        Err(VaultError::InvalidPasswordOrKey)
    );
    service
        .authenticate_session(&session, "copy password")
        .unwrap();
}

#[test]
fn replacement_rechecks_fingerprint_and_preserves_external_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.snotes");
    fs::write(&path, b"original").unwrap();
    let expected = super::fingerprint(b"original").unwrap();
    fs::write(&path, b"external update").unwrap();
    assert_eq!(
        super::atomic_write_checked(&path, b"replacement", None, true, Some(expected)),
        Err(VaultError::FileConflict)
    );
    assert_eq!(fs::read(&path).unwrap(), b"external update");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn clearing_a_secret_also_wipes_previously_truncated_bytes() {
    let mut secret = String::from("secret previously truncated suffix");
    let initialized_len = secret.len();
    secret.truncate(6);
    super::clear_secret(&mut secret);
    assert!(secret.is_empty());
    // The String still owns the allocation; all these bytes were initialized.
    let bytes = unsafe { std::slice::from_raw_parts(secret.as_ptr(), initialized_len) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}
