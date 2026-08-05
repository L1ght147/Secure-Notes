use super::{
    HEADER_LEN, KdfProfile, Note, PAYLOAD_LENGTH_OFFSET, PAYLOAD_LIMIT, Vault, VaultCodec,
    VaultError, WRAPPED_KEY_OFFSET, validate_password,
};
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
fn rewrapping_key_changes_password_without_changing_data_key() {
    let codec = VaultCodec::new(KdfProfile::testing());
    let (container, original_key) = codec.encrypt_new(&sample_vault(), "old password").unwrap();

    let changed = codec
        .change_password(&container, &original_key, "new password")
        .unwrap();
    let (_, opened_key) = codec.decrypt(&changed, "new password").unwrap();

    assert_eq!(opened_key.expose(), original_key.expose());
    assert!(matches!(
        codec.decrypt(&changed, "old password"),
        Err(VaultError::InvalidPasswordOrKey)
    ));
}
