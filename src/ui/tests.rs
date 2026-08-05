use super::{
    i18n::{Language, TextKey},
    model::{
        AutoLockTimer, close_requires_confirmation, create_note, delete_note,
        lock_for_session_event, rename_note, search_notes,
    },
    settings::AppSettings,
};
use crate::platform::SessionEvent;
use crate::vault::Vault;
use std::time::Duration;

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

#[test]
fn auto_lock_timer_resets_on_activity_and_fires_at_boundary() {
    let mut timer = AutoLockTimer::new(Duration::from_secs(10));
    timer.record_activity(Duration::from_secs(100));

    assert!(!timer.expired(Duration::from_secs(109)));
    assert!(timer.expired(Duration::from_secs(110)));

    timer.record_activity(Duration::from_secs(110));
    assert!(!timer.expired(Duration::from_secs(119)));
}

#[test]
fn windows_lock_and_sleep_follow_setting_but_resume_does_not_lock() {
    assert!(lock_for_session_event(SessionEvent::Locked, true));
    assert!(lock_for_session_event(SessionEvent::Sleeping, true));
    assert!(!lock_for_session_event(SessionEvent::Resumed, true));
    assert!(!lock_for_session_event(SessionEvent::Locked, false));
}

#[test]
fn close_confirmation_is_required_for_dirty_and_soft_locked_sessions() {
    assert!(!close_requires_confirmation(false, false));
    assert!(close_requires_confirmation(true, false));
    assert!(close_requires_confirmation(false, true));
}
