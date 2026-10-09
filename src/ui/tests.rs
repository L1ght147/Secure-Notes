use super::{
    PendingVaultKeyboardCommand, VaultAction, error_text,
    i18n::{Language, TextKey},
    layout::{WorkspaceMetrics, center_window_in_display},
    model::{
        AutoLockTimer, close_requires_confirmation, create_note, delete_note,
        lock_for_session_event, rename_note, search_notes_sorted,
    },
    password_enter_submits, pending_vault_keyboard_command,
    settings::{AppSettings, NoteSortOrder, ThemePreference},
    theme,
};
use crate::platform::SessionEvent;
use crate::vault::VaultError;
use crate::vault::{Note, Vault};
use std::time::Duration;
use uuid::Uuid;

#[test]
fn vault_dialog_keyboard_shortcuts_open_or_cancel_as_expected() {
    assert_eq!(
        pending_vault_keyboard_command(VaultAction::Open, true, true, false),
        PendingVaultKeyboardCommand::Submit
    );
    assert_eq!(
        pending_vault_keyboard_command(VaultAction::Open, true, true, true),
        PendingVaultKeyboardCommand::Cancel
    );
    assert_eq!(
        pending_vault_keyboard_command(VaultAction::Open, false, true, false),
        PendingVaultKeyboardCommand::None
    );
    assert_eq!(
        pending_vault_keyboard_command(VaultAction::Create, true, true, false),
        PendingVaultKeyboardCommand::None
    );
}

#[test]
fn locked_vault_accepts_enter_only_with_a_password() {
    assert!(password_enter_submits(true, true));
    assert!(!password_enter_submits(false, true));
    assert!(!password_enter_submits(true, false));
}

#[test]
fn note_crud_and_search_operate_on_ram_vault() {
    let mut vault = Vault::default();
    let first = create_note(&mut vault, 100);
    rename_note(&mut vault, first, "Планы", 101).unwrap();
    vault.notes[0].body = "Купить молоко".into();
    let second = create_note(&mut vault, 102);
    rename_note(&mut vault, second, "Work", 103).unwrap();

    assert_eq!(
        search_notes_sorted(&vault, "молоко", NoteSortOrder::ModifiedNewestFirst),
        vec![first]
    );
    assert_eq!(
        search_notes_sorted(&vault, "WORK", NoteSortOrder::ModifiedNewestFirst),
        vec![second]
    );
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
fn sorted_search_results_follow_the_selected_order() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let third = Uuid::from_u128(3);
    let vault = Vault {
        schema_version: 1,
        notes: vec![
            Note {
                id: first,
                title: "Zebra".into(),
                body: "match".into(),
                modified_at: 20,
            },
            Note {
                id: second,
                title: "Apple".into(),
                body: "match".into(),
                modified_at: 30,
            },
            Note {
                id: third,
                title: "Apple".into(),
                body: "match".into(),
                modified_at: 10,
            },
        ],
    };

    assert_eq!(
        search_notes_sorted(&vault, "match", NoteSortOrder::ModifiedNewestFirst),
        vec![second, first, third]
    );
    assert_eq!(
        search_notes_sorted(&vault, "match", NoteSortOrder::TitleAscending),
        vec![second, third, first]
    );
}

#[test]
fn settings_persist_only_non_secret_preferences() {
    let settings = AppSettings {
        language: Language::Russian,
        auto_lock_minutes: 12,
        lock_on_session_events: false,
        theme_preference: ThemePreference::System,
        note_sort_order: NoteSortOrder::ModifiedNewestFirst,
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
fn legacy_settings_default_to_system_theme_and_persist_theme_choice() {
    let legacy = r#"{"language":"English","auto_lock_minutes":5,"lock_on_session_events":true}"#;
    assert_eq!(
        serde_json::from_str::<AppSettings>(legacy)
            .unwrap()
            .theme_preference,
        ThemePreference::System
    );
    assert_eq!(
        serde_json::from_str::<AppSettings>(legacy)
            .unwrap()
            .note_sort_order,
        NoteSortOrder::ModifiedNewestFirst
    );

    let settings = AppSettings {
        language: Language::English,
        auto_lock_minutes: 5,
        lock_on_session_events: true,
        theme_preference: ThemePreference::Dark,
        note_sort_order: NoteSortOrder::TitleAscending,
    };
    assert_eq!(
        serde_json::from_str::<AppSettings>(&serde_json::to_string(&settings).unwrap())
            .unwrap()
            .theme_preference,
        ThemePreference::Dark
    );
}

#[test]
fn workspace_metrics_keep_the_mock_sidebar_and_readable_editor_column() {
    let metrics = WorkspaceMetrics::for_window_width(1120.0);

    assert_eq!(metrics.toolbar_height, 68.0);
    assert_eq!(metrics.sidebar_width, 286.0);
    assert_eq!(metrics.editor_width, 684.0);
    assert_eq!(metrics.editor_top_padding, 42.0);
    assert_eq!(WorkspaceMetrics::SETTINGS_CLOSE_HITBOX, 40.0);
    assert_eq!(WorkspaceMetrics::WELCOME_ACTIONS_SIZE, [520.0, 86.0]);
    assert_eq!(WorkspaceMetrics::LOCKED_PANEL_SIZE, [400.0, 208.0]);
    assert_eq!(WorkspaceMetrics::sidebar_controls_width(278.0), 270.0);
    assert_eq!(WorkspaceMetrics::SIDEBAR_FILTER_HEIGHT, 34.0);

    let compact = WorkspaceMetrics::for_window_width(760.0);
    assert_eq!(compact.sidebar_width, 224.0);
    assert!(compact.editor_width >= 420.0);
}

#[test]
fn initial_window_position_is_centered_on_the_display() {
    assert_eq!(
        center_window_in_display([1920.0, 1080.0], [1120.0, 720.0]),
        [400.0, 180.0]
    );
}

#[test]
fn wrong_password_message_does_not_mention_key_damage() {
    assert_eq!(
        error_text(Language::Russian, &VaultError::InvalidPasswordOrKey),
        "Неверный пароль."
    );
}

#[test]
fn theme_uses_readable_text_for_noninteractive_widgets() {
    let light = theme::visuals_for(false);
    let dark = theme::visuals_for(true);

    assert_eq!(
        light.widgets.noninteractive.fg_stroke.color,
        eframe::egui::Color32::from_rgb(18, 18, 18)
    );
    assert_eq!(
        dark.widgets.noninteractive.fg_stroke.color,
        eframe::egui::Color32::from_rgb(245, 245, 245)
    );
    assert_eq!(
        light.strong_text_color(),
        eframe::egui::Color32::from_rgb(18, 18, 18)
    );
    assert_eq!(
        dark.strong_text_color(),
        eframe::egui::Color32::from_rgb(245, 245, 245)
    );
    assert_eq!(
        dark.widgets.open.fg_stroke.color,
        eframe::egui::Color32::from_rgb(245, 245, 245)
    );
    assert_eq!(
        dark.widgets.open.bg_fill,
        eframe::egui::Color32::from_rgb(42, 42, 42)
    );
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

fn app_with_session(session: crate::vault::VaultSession) -> super::SecureNotesApp {
    super::SecureNotesApp {
        egui_ctx: eframe::egui::Context::default(),
        session_monitor_available: false,
        state: super::SessionState::Unlocked(session),
        service: crate::vault::VaultService::testing(),
        platform: crate::platform::NativePlatformSecurity,
        session_events: std::sync::mpsc::channel().1,
        settings: AppSettings::default(),
        pending_vault: None,
        password: String::new(),
        confirm_password: String::new(),
        new_password: String::new(),
        confirm_new_password: String::new(),
        copy_password: String::new(),
        search: String::new(),
        selected_note: None,
        delete_confirmation: None,
        show_settings: false,
        show_change_password: false,
        show_save_copy: false,
        close_prompt: false,
        error: None,
        transient_message: None,
        started_at: std::time::Instant::now(),
        auto_lock: AutoLockTimer::new(Duration::from_secs(300)),
    }
}

#[test]
fn soft_locked_copy_rejects_an_unrelated_password() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.snotes");
    let service = crate::vault::VaultService::testing();
    let mut session = service.create(&path, "original password").unwrap();
    session.vault.notes.push(Note {
        id: Uuid::new_v4(),
        title: "private".into(),
        body: "secret body".into(),
        modified_at: 0,
    });
    session.dirty = true;
    let mut app = app_with_session(session);
    std::fs::remove_file(&path).unwrap();
    app.lock_current();
    assert!(matches!(app.state, super::SessionState::SoftLocked { .. }));
    app.copy_password = "attacker password".into();
    let copy = directory.path().join("copy.snotes");
    assert_eq!(
        app.save_copy_current(copy.clone()),
        Err(VaultError::InvalidPasswordOrKey)
    );
    assert!(!copy.exists());
    assert!(matches!(app.state, super::SessionState::SoftLocked { .. }));
}

#[test]
fn locking_clears_password_fields_search_and_secret_dialogs() {
    let directory = tempfile::tempdir().unwrap();
    let session = crate::vault::VaultService::testing()
        .create(directory.path().join("vault.snotes"), "original password")
        .unwrap();
    let mut app = app_with_session(session);
    app.search = "secret query".into();
    app.new_password = "new secret".into();
    app.confirm_new_password = "new secret".into();
    app.copy_password = "copy secret".into();
    app.show_change_password = true;
    app.show_save_copy = true;
    app.delete_confirmation = Some(Uuid::new_v4());
    app.lock_current();
    assert!(app.search.is_empty());
    assert!(app.new_password.is_empty());
    assert!(app.confirm_new_password.is_empty());
    assert!(app.copy_password.is_empty());
    assert!(!app.show_change_password);
    assert!(!app.show_save_copy);
    assert!(app.delete_confirmation.is_none());
}

#[test]
fn soft_locked_copy_recovers_deleted_vault_only_with_original_password() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.snotes");
    let service = crate::vault::VaultService::testing();
    let mut session = service.create(&path, "original password").unwrap();
    session.vault.notes.push(Note {
        id: Uuid::new_v4(),
        title: "private".into(),
        body: "secret body".into(),
        modified_at: 0,
    });
    session.dirty = true;
    let mut app = app_with_session(session);
    std::fs::remove_file(&path).unwrap();
    app.lock_current();
    app.password = "original password".into();
    app.copy_password = "replacement password".into();
    let copy = directory.path().join("copy.snotes");
    assert_eq!(app.save_copy_current(copy.clone()), Ok(true));
    assert!(matches!(app.state, super::SessionState::Locked { .. }));
    assert_eq!(
        service
            .open(&copy, "replacement password")
            .unwrap()
            .vault
            .notes[0]
            .body,
        "secret body"
    );
    assert!(app.password.is_empty());
    assert!(app.copy_password.is_empty());
}

#[test]
fn locking_removes_editor_undo_history() {
    use eframe::egui::{
        Id,
        text::{CCursor, CCursorRange},
        text_edit::TextEditState,
    };
    let directory = tempfile::tempdir().unwrap();
    let session = crate::vault::VaultService::testing()
        .create(directory.path().join("vault.snotes"), "original password")
        .unwrap();
    let mut app = app_with_session(session);
    let id = Id::new("test-editor");
    let mut state = TextEditState::default();
    let mut undoer = state.undoer();
    undoer.feed_state(
        0.0,
        &(CCursorRange::one(CCursor::new(0)), "secret body".to_owned()),
    );
    state.set_undoer(undoer);
    state.store(&app.egui_ctx, id);
    assert!(TextEditState::load(&app.egui_ctx, id).is_some());
    app.lock_current();
    assert!(TextEditState::load(&app.egui_ctx, id).is_none());
}

#[test]
fn failed_password_change_clears_password_undo_history() {
    use eframe::egui::{
        Id,
        text::{CCursor, CCursorRange},
        text_edit::TextEditState,
    };
    for (password, confirmation, remove_original) in [
        ("replacement password", "mismatch", false),
        ("x", "x", false),
        ("replacement password", "replacement password", true),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("vault.snotes");
        let session = crate::vault::VaultService::testing()
            .create(&path, "original password")
            .unwrap();
        let mut app = app_with_session(session);
        app.new_password = password.into();
        app.confirm_new_password = confirmation.into();
        let id = Id::new("new-password");
        let mut state = TextEditState::default();
        let mut undoer = state.undoer();
        undoer.feed_state(
            0.0,
            &(CCursorRange::one(CCursor::new(0)), password.to_owned()),
        );
        state.set_undoer(undoer);
        state.store(&app.egui_ctx, id);
        if remove_original {
            std::fs::remove_file(&path).unwrap();
        }
        assert!(!app.change_password_current());
        assert!(app.new_password.is_empty());
        assert!(app.confirm_new_password.is_empty());
        assert!(TextEditState::load(&app.egui_ctx, id).is_none());
    }
}
