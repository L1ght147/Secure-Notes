use super::{
    i18n::{Language, TextKey},
    model::{create_note, delete_note, rename_note, search_notes},
    settings::AppSettings,
};
use crate::vault::Vault;

#[test]
fn note_crud_and_search_operate_on_ram_vault() {
    let mut vault = Vault::default();
    let first = create_note(&mut vault, 100);
    rename_note(&mut vault, first, "Планы", 101).unwrap();
    vault.notes[0].body = "Купить молоко".into();
    let second = create_note(&mut vault, 102);
    rename_note(&mut vault, second, "Work", 103).unwrap();

    assert_eq!(search_notes(&vault, "молоко"), vec![first]);
    assert_eq!(search_notes(&vault, "WORK"), vec![second]);
    assert!(delete_note(&mut vault, first));
    assert_eq!(vault.notes.len(), 1);
}

#[test]
fn language_defaults_from_locale_and_switches_immediately() {
    assert_eq!(Language::from_locale("ru-RU"), Language::Russian);
    assert_eq!(Language::from_locale("en-US"), Language::English);
    assert_eq!(Language::Russian.text(TextKey::Save), "Сохранить");
    assert_eq!(Language::English.text(TextKey::Save), "Save");
}

#[test]
fn settings_persist_only_non_secret_preferences() {
    let settings = AppSettings {
        language: Language::Russian,
        auto_lock_minutes: 12,
        lock_on_session_events: false,
    };
    let json = serde_json::to_string(&settings).unwrap();

    assert_eq!(
        serde_json::from_str::<AppSettings>(&json).unwrap(),
        settings
    );
    assert!(!json.contains("path"));
    assert!(!json.contains("search"));
    assert!(!json.contains("editor"));
    assert!(!json.contains("password"));
}
