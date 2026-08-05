use super::{
    FaultPoint, HEADER_LEN, KdfProfile, Note, PAYLOAD_LENGTH_OFFSET, PAYLOAD_LIMIT, Vault,
    VaultCodec, VaultError, VaultService, WRAPPED_KEY_OFFSET, validate_password,
};
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
