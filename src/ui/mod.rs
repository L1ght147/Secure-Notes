//! Native application interface.

mod i18n;
mod layout;
mod model;
mod settings;
mod theme;

use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self, Color32, RichText};
use uuid::Uuid;

use crate::{
    platform::{NativePlatformSecurity, PlatformSecurity, SessionEvent},
    vault::{VaultError, VaultService, VaultSession},
};

use self::{
    i18n::{Language, TextKey},
    layout::WorkspaceMetrics,
    model::{
        AutoLockTimer, close_requires_confirmation, create_note, delete_note,
        lock_for_session_event, search_notes,
    },
    settings::{AppSettings, ThemePreference},
};

#[cfg(test)]
mod tests;

pub enum SessionState {
    Welcome,
    Unlocked(VaultSession),
    Locked {
        path: PathBuf,
    },
    SoftLocked {
        session: VaultSession,
        save_error: VaultError,
    },
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Secure Notes",
        options,
        Box::new(|cc| Ok(Box::new(SecureNotesApp::new(cc)))),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VaultAction {
    Create,
    Open,
}

struct PendingVault {
    action: VaultAction,
    path: PathBuf,
}

struct SecureNotesApp {
    state: SessionState,
    service: VaultService,
    platform: NativePlatformSecurity,
    session_events: Receiver<SessionEvent>,
    settings: AppSettings,
    pending_vault: Option<PendingVault>,
    password: String,
    confirm_password: String,
    new_password: String,
    confirm_new_password: String,
    copy_password: String,
    search: String,
    selected_note: Option<Uuid>,
    delete_confirmation: Option<Uuid>,
    show_settings: bool,
    show_change_password: bool,
    show_save_copy: bool,
    close_prompt: bool,
    error: Option<VaultError>,
    transient_message: Option<&'static str>,
    started_at: Instant,
    auto_lock: AutoLockTimer,
}

impl SecureNotesApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let platform = NativePlatformSecurity;
        let session_events = platform
            .subscribe_session_events()
            .unwrap_or_else(|_| std::sync::mpsc::channel().1);
        let settings = AppSettings::load();
        theme::apply(&cc.egui_ctx, settings.theme_preference);
        let timeout = Duration::from_secs(u64::from(settings.auto_lock_minutes) * 60);
        Self {
            state: SessionState::Welcome,
            service: VaultService::default(),
            platform,
            session_events,
            settings,
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
            started_at: Instant::now(),
            auto_lock: AutoLockTimer::new(timeout),
        }
    }

    fn language(&self) -> Language {
        self.settings.language
    }

    fn handle_shortcuts_and_close(&mut self, ctx: &egui::Context) {
        let save_requested =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S));
        if save_requested {
            self.save_current();
        }

        if ctx.input(|input| input.viewport().close_requested()) {
            let (dirty, soft_locked) = match &self.state {
                SessionState::Unlocked(session) => (session.dirty, false),
                SessionState::SoftLocked { .. } => (false, true),
                _ => (false, false),
            };
            let needs_prompt = close_requires_confirmation(dirty, soft_locked);
            if needs_prompt {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_prompt = true;
            }
        }
    }

    fn save_current(&mut self) -> bool {
        let SessionState::Unlocked(session) = &mut self.state else {
            return false;
        };
        match self.service.save(session) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.show_save_copy = error == VaultError::FileConflict;
                self.error = Some(error);
                false
            }
        }
    }

    fn lock_current(&mut self) {
        let state = std::mem::replace(&mut self.state, SessionState::Welcome);
        let SessionState::Unlocked(mut session) = state else {
            self.state = state;
            return;
        };
        if session.dirty
            && let Err(save_error) = self.service.save(&mut session)
        {
            self.state = SessionState::SoftLocked {
                session,
                save_error,
            };
            return;
        }
        self.state = SessionState::Locked {
            path: session.path.clone(),
        };
    }

    fn retry_soft_locked_save(&mut self) -> bool {
        let state = std::mem::replace(&mut self.state, SessionState::Welcome);
        let SessionState::SoftLocked { mut session, .. } = state else {
            self.state = state;
            return false;
        };
        match self
            .service
            .save_after_reauthentication(&mut session, &self.password)
        {
            Ok(()) => {
                self.state = SessionState::Locked {
                    path: session.path.clone(),
                };
                self.error = None;
                clear_secret(&mut self.password);
                true
            }
            Err(error) => {
                self.state = SessionState::SoftLocked {
                    session,
                    save_error: error.clone(),
                };
                self.error = Some(error);
                clear_secret(&mut self.password);
                false
            }
        }
    }

    fn save_copy_current(&mut self, path: PathBuf) -> Result<bool, VaultError> {
        let was_soft_locked = matches!(self.state, SessionState::SoftLocked { .. });
        match &mut self.state {
            SessionState::Unlocked(session) | SessionState::SoftLocked { session, .. } => {
                self.service.save_copy(session, path, &self.copy_password)?;
            }
            _ => return Ok(false),
        }
        if was_soft_locked {
            let state = std::mem::replace(&mut self.state, SessionState::Welcome);
            if let SessionState::SoftLocked { session, .. } = state {
                self.state = SessionState::Locked {
                    path: session.path.clone(),
                };
            }
        }
        Ok(true)
    }

    fn render_welcome(&mut self, ui: &mut egui::Ui) {
        self.render_language_picker(ui);
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(520.0);
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(egui::Margin::same(34))
                .show(ui, |ui| {
                    ui.set_min_width(400.0);
                    ui.vertical_centered(|ui| {
                        ui.heading(
                            RichText::new(self.language().text(TextKey::WelcomeTitle))
                                .size(30.0)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(self.language().text(TextKey::WelcomeBody));
                        ui.add_space(28.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [190.0, 46.0],
                                    egui::Button::new(self.language().text(TextKey::NewVault)),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Secure Notes", &["snotes"])
                                    .set_file_name("notes.snotes")
                                    .save_file()
                            {
                                self.pending_vault = Some(PendingVault {
                                    action: VaultAction::Create,
                                    path,
                                });
                                self.error = None;
                            }
                            if ui
                                .add_sized(
                                    [190.0, 46.0],
                                    egui::Button::new(self.language().text(TextKey::OpenVault)),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Secure Notes", &["snotes"])
                                    .pick_file()
                            {
                                self.pending_vault = Some(PendingVault {
                                    action: VaultAction::Open,
                                    path,
                                });
                                self.error = None;
                            }
                        });
                    });
                });
        });
        self.render_pending_vault(ui.ctx());
    }

    fn render_pending_vault(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending_vault else {
            return;
        };
        let action = pending.action;
        let path = pending.path.clone();
        let title = match action {
            VaultAction::Create => self.language().text(TextKey::NewVault),
            VaultAction::Open => self.language().text(TextKey::OpenVault),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(390.0);
                ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                ui.add_space(8.0);
                ui.label(self.language().text(TextKey::Password));
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                if action == VaultAction::Create {
                    ui.label(self.language().text(TextKey::ConfirmPassword));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.confirm_password)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                }
                self.render_error(ui);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let submit_label = match action {
                        VaultAction::Create => self.language().text(TextKey::Create),
                        VaultAction::Open => self.language().text(TextKey::Open),
                    };
                    let submit = ui
                        .add_enabled(!self.password.is_empty(), egui::Button::new(submit_label))
                        .clicked();
                    if ui.button(self.language().text(TextKey::Cancel)).clicked() {
                        self.pending_vault = None;
                        clear_secret(&mut self.password);
                        clear_secret(&mut self.confirm_password);
                        self.error = None;
                    } else if submit {
                        if action == VaultAction::Create && self.password != self.confirm_password {
                            self.error = None;
                            self.transient_message =
                                Some(self.language().text(TextKey::PasswordMismatch));
                            return;
                        }
                        let result = match action {
                            VaultAction::Create => self.service.create(&path, &self.password),
                            VaultAction::Open => self.service.open(&path, &self.password),
                        };
                        match result {
                            Ok(session) => {
                                self.selected_note =
                                    session.vault.notes.first().map(|note| note.id);
                                self.state = SessionState::Unlocked(session);
                                self.pending_vault = None;
                                self.error = None;
                                self.transient_message = None;
                                clear_secret(&mut self.password);
                                clear_secret(&mut self.confirm_password);
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                });
            });
    }

    fn render_unlocked(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let palette = theme::palette_for(ui.visuals().dark_mode);
        let SessionState::Unlocked(session) = &mut self.state else {
            return;
        };
        if self.selected_note.is_none() {
            self.selected_note = session.vault.notes.first().map(|note| note.id);
        }

        let mut save_clicked = false;
        let mut lock_clicked = false;
        let mut new_clicked = false;
        let mut settings_clicked = false;
        let mut password_clicked = false;

        let metrics = WorkspaceMetrics::for_window_width(ui.available_width());
        let selected_title = self
            .selected_note
            .and_then(|id| session.vault.notes.iter().find(|note| note.id == id))
            .map(|note| note.title.trim())
            .filter(|title| !title.is_empty())
            .unwrap_or(language.text(TextKey::Untitled));
        let status = if session.dirty {
            language.text(TextKey::Modified)
        } else {
            language.text(TextKey::Saved)
        };
        egui::Panel::top("workspace-toolbar")
            .exact_size(metrics.toolbar_height)
            .show(ui, |ui| {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(selected_title).size(16.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lock_clicked = ui.button(language.text(TextKey::Lock)).clicked();
                        password_clicked =
                            ui.button(language.text(TextKey::ChangePassword)).clicked();
                        settings_clicked = ui.button(language.text(TextKey::Settings)).clicked();
                        save_clicked = ui
                            .add_enabled(
                                session.dirty,
                                egui::Button::new(language.text(TextKey::Save)),
                            )
                            .clicked();
                        ui.add_space(8.0);
                        ui.label(RichText::new(status).small().color(if session.dirty {
                            Color32::from_rgb(181, 115, 47)
                        } else {
                            palette.muted
                        }));
                    });
                });
            });

        egui::Panel::left("workspace-sidebar")
            .exact_size(metrics.sidebar_width)
            .show(ui, |ui| {
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(language.text(TextKey::AppTitle))
                            .size(16.0)
                            .strong(),
                    );
                });
                ui.add_space(20.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(language.text(TextKey::Search))
                        .desired_width(f32::INFINITY),
                );
                new_clicked = ui
                    .add_sized(
                        [ui.available_width(), 38.0],
                        egui::Button::new(
                            RichText::new(format!("+ {}", language.text(TextKey::NewNote)))
                                .color(palette.canvas),
                        )
                        .fill(palette.accent),
                    )
                    .clicked();
                ui.add_space(14.0);
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        language.text(TextKey::NoteBody).to_uppercase(),
                        session.vault.notes.len()
                    ))
                    .size(10.0)
                    .strong()
                    .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(7.0);
                let ids = search_notes(&session.vault, &self.search);
                if ids.is_empty() && !session.vault.notes.is_empty() {
                    ui.weak(language.text(TextKey::NoResults));
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for id in ids {
                        let Some(note) = session.vault.notes.iter().find(|note| note.id == id)
                        else {
                            continue;
                        };
                        let title = if note.title.trim().is_empty() {
                            language.text(TextKey::Untitled)
                        } else {
                            note.title.as_str()
                        };
                        let selected = self.selected_note == Some(id);
                        if ui
                            .add_sized(
                                [ui.available_width(), 52.0],
                                egui::Button::new(RichText::new(title).size(14.0))
                                    .fill(if selected {
                                        palette.selected
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            self.selected_note = Some(id);
                        }
                        ui.add_space(3.0);
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(58.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(metrics.editor_width);
                let Some(selected) = self.selected_note else {
                    render_empty(ui, language);
                    return;
                };
                let Some(note) = session
                    .vault
                    .notes
                    .iter_mut()
                    .find(|note| note.id == selected)
                else {
                    render_empty(ui, language);
                    return;
                };
                ui.label(
                    RichText::new("ЛИЧНОЕ · ЛОКАЛЬНАЯ ЗАМЕТКА")
                        .size(10.0)
                        .strong()
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(16.0);
                let title_response = ui.add(
                    egui::TextEdit::singleline(&mut note.title)
                        .hint_text(language.text(TextKey::NoteTitle))
                        .font(egui::FontId::proportional(34.0))
                        .desired_width(f32::INFINITY)
                        .frame(
                            egui::Frame::new()
                                .fill(palette.surface)
                                .stroke(egui::Stroke::new(1.0, palette.border))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(14, 12)),
                        ),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(if session.dirty {
                        language.text(TextKey::Modified)
                    } else {
                        language.text(TextKey::Saved)
                    })
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(28.0);
                let body_response = ui.add_sized(
                    [
                        ui.available_width(),
                        (ui.available_height() - 44.0).max(120.0),
                    ],
                    egui::TextEdit::multiline(&mut note.body)
                        .hint_text(language.text(TextKey::NoteBody))
                        .font(egui::FontId::proportional(16.0))
                        .frame(
                            egui::Frame::new()
                                .fill(palette.surface)
                                .stroke(egui::Stroke::new(1.0, palette.border))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::same(14)),
                        ),
                );
                if title_response.changed() || body_response.changed() {
                    note.modified_at = unix_timestamp();
                    session.dirty = true;
                }
                ui.add_space(8.0);
                if ui.small_button(language.text(TextKey::Delete)).clicked() {
                    self.delete_confirmation = Some(selected);
                }
            });
        });

        if new_clicked {
            let id = create_note(&mut session.vault, unix_timestamp());
            self.selected_note = Some(id);
            self.search.clear();
            session.dirty = true;
        }
        if save_clicked {
            self.save_current();
        }
        if lock_clicked {
            self.lock_current();
        }
        if settings_clicked {
            self.show_settings = true;
        }
        if password_clicked {
            self.show_change_password = true;
        }
    }

    fn render_locked(&mut self, ui: &mut egui::Ui, path: PathBuf) {
        let password_hint = self.language().text(TextKey::Password);
        self.render_language_picker(ui);
        ui.add_space(90.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(400.0);
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .stroke(egui::Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(egui::Margin::same(30))
                .show(ui, |ui| {
                    ui.heading(self.language().text(TextKey::LockedTitle));
                    ui.label(self.language().text(TextKey::LockedBody));
                    ui.add_space(14.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(true)
                            .hint_text(password_hint)
                            .desired_width(f32::INFINITY),
                    );
                    if ui
                        .add_enabled(
                            !self.password.is_empty(),
                            egui::Button::new(self.language().text(TextKey::Unlock)),
                        )
                        .clicked()
                    {
                        match self.service.open(&path, &self.password) {
                            Ok(session) => {
                                self.selected_note =
                                    session.vault.notes.first().map(|note| note.id);
                                self.state = SessionState::Unlocked(session);
                                self.error = None;
                                clear_secret(&mut self.password);
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                    self.render_error(ui);
                });
        });
    }

    fn render_language_picker(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("language")
                .selected_text(self.settings.language.name())
                .show_ui(ui, |ui| {
                    let mut changed = false;
                    changed |= ui
                        .selectable_value(
                            &mut self.settings.language,
                            Language::Russian,
                            Language::Russian.name(),
                        )
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut self.settings.language,
                            Language::English,
                            Language::English.name(),
                        )
                        .changed();
                    if changed {
                        self.settings.save();
                    }
                });
        });
    }

    fn render_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let language = self.language();
        let mut open = self.show_settings;
        let previous = self.settings.clone();
        egui::Window::new(language.text(TextKey::Settings))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.label(language.text(TextKey::Language));
                egui::ComboBox::from_id_salt("settings-language")
                    .selected_text(self.settings.language.name())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.language,
                            Language::Russian,
                            Language::Russian.name(),
                        );
                        ui.selectable_value(
                            &mut self.settings.language,
                            Language::English,
                            Language::English.name(),
                        );
                    });
                ui.add_space(10.0);
                ui.label(match language {
                    Language::Russian => "Тема",
                    Language::English => "Theme",
                });
                egui::ComboBox::from_id_salt("settings-theme")
                    .selected_text(match self.settings.theme_preference {
                        ThemePreference::System => match language {
                            Language::Russian => "Как в системе",
                            Language::English => "System",
                        },
                        ThemePreference::Light => match language {
                            Language::Russian => "Светлая",
                            Language::English => "Light",
                        },
                        ThemePreference::Dark => match language {
                            Language::Russian => "Тёмная",
                            Language::English => "Dark",
                        },
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.theme_preference,
                            ThemePreference::System,
                            match language {
                                Language::Russian => "Как в системе",
                                Language::English => "System",
                            },
                        );
                        ui.selectable_value(
                            &mut self.settings.theme_preference,
                            ThemePreference::Light,
                            match language {
                                Language::Russian => "Светлая",
                                Language::English => "Light",
                            },
                        );
                        ui.selectable_value(
                            &mut self.settings.theme_preference,
                            ThemePreference::Dark,
                            match language {
                                Language::Russian => "Тёмная",
                                Language::English => "Dark",
                            },
                        );
                    });
                ui.separator();
                ui.label(language.text(TextKey::AutoLock));
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(
                        &mut self.settings.auto_lock_minutes,
                        1..=120,
                    ));
                    ui.label(language.text(TextKey::Minutes));
                });
                ui.checkbox(
                    &mut self.settings.lock_on_session_events,
                    language.text(TextKey::LockOnWindows),
                );
            });
        if self.settings != previous {
            self.settings.save();
            theme::apply(ctx, self.settings.theme_preference);
        }
        self.show_settings = open;
    }

    fn render_change_password(&mut self, ctx: &egui::Context) {
        if !self.show_change_password {
            return;
        }
        let language = self.language();
        let mut open = self.show_change_password;
        let mut submit = false;
        egui::Window::new(language.text(TextKey::ChangePassword))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.label(language.text(TextKey::NewPassword));
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                ui.label(language.text(TextKey::ConfirmPassword));
                ui.add(
                    egui::TextEdit::singleline(&mut self.confirm_new_password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                self.render_error(ui);
                submit = ui
                    .add_enabled(
                        !self.new_password.is_empty(),
                        egui::Button::new(language.text(TextKey::Change)),
                    )
                    .clicked();
            });
        if submit {
            if self.new_password != self.confirm_new_password {
                self.transient_message = Some(language.text(TextKey::PasswordMismatch));
            } else if let SessionState::Unlocked(session) = &mut self.state {
                match self.service.change_password(session, &self.new_password) {
                    Ok(()) => {
                        open = false;
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            clear_secret(&mut self.new_password);
            clear_secret(&mut self.confirm_new_password);
        }
        self.show_change_password = open;
    }

    fn render_delete_confirmation(&mut self, ctx: &egui::Context) {
        let Some(id) = self.delete_confirmation else {
            return;
        };
        let language = self.language();
        egui::Window::new(language.text(TextKey::Delete))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(language.text(TextKey::DeleteQuestion));
                ui.horizontal(|ui| {
                    if ui.button(language.text(TextKey::Delete)).clicked() {
                        if let SessionState::Unlocked(session) = &mut self.state
                            && delete_note(&mut session.vault, id)
                        {
                            session.dirty = true;
                            self.selected_note = session.vault.notes.first().map(|note| note.id);
                        }
                        self.delete_confirmation = None;
                    }
                    if ui.button(language.text(TextKey::Cancel)).clicked() {
                        self.delete_confirmation = None;
                    }
                });
            });
    }

    fn render_save_copy(&mut self, ctx: &egui::Context) {
        if !self.show_save_copy {
            return;
        }
        let language = self.language();
        let mut open = self.show_save_copy;
        let mut choose = false;
        egui::Window::new(language.text(TextKey::SaveCopy))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.label(language.text(TextKey::Password));
                ui.add(
                    egui::TextEdit::singleline(&mut self.copy_password)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                choose = ui
                    .add_enabled(
                        !self.copy_password.is_empty(),
                        egui::Button::new(language.text(TextKey::SaveCopy)),
                    )
                    .clicked();
            });
        if choose
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Secure Notes", &["snotes"])
                .set_file_name("notes-copy.snotes")
                .save_file()
        {
            match self.save_copy_current(path) {
                Ok(true) => {
                    open = false;
                    self.error = None;
                }
                Ok(false) => {}
                Err(error) => self.error = Some(error),
            }
            clear_secret(&mut self.copy_password);
        }
        self.show_save_copy = open;
    }

    fn render_close_prompt(&mut self, ctx: &egui::Context) {
        if !self.close_prompt {
            return;
        }
        let language = self.language();
        egui::Window::new(language.text(TextKey::AppTitle))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(language.text(TextKey::CloseUnsaved));
                let soft_locked = matches!(self.state, SessionState::SoftLocked { .. });
                if soft_locked {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(true)
                            .hint_text(language.text(TextKey::Password))
                            .desired_width(f32::INFINITY),
                    );
                }
                ui.horizontal(|ui| {
                    let save_clicked = ui
                        .add_enabled(
                            !soft_locked || !self.password.is_empty(),
                            egui::Button::new(language.text(TextKey::Save)),
                        )
                        .clicked();
                    let saved = if soft_locked {
                        save_clicked && self.retry_soft_locked_save()
                    } else {
                        save_clicked && self.save_current()
                    };
                    if saved {
                        self.close_prompt = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(language.text(TextKey::Discard)).clicked() {
                        self.state = SessionState::Welcome;
                        clear_secret(&mut self.password);
                        self.close_prompt = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(language.text(TextKey::Cancel)).clicked() {
                        self.close_prompt = false;
                    }
                });
                self.render_error(ui);
            });
    }

    fn render_soft_locked(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        let mut retry = false;
        let mut discard = false;
        let mut save_copy = false;
        ui.add_space(90.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(500.0);
            ui.heading(language.text(TextKey::SoftLockTitle));
            ui.label(language.text(TextKey::SoftLockBody));
            if let SessionState::SoftLocked { save_error, .. } = &self.state {
                ui.colored_label(
                    Color32::from_rgb(205, 80, 65),
                    error_text(language, save_error),
                );
            }
            ui.add_space(12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.password)
                    .password(true)
                    .hint_text(language.text(TextKey::Password))
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                retry = ui
                    .add_enabled(
                        !self.password.is_empty(),
                        egui::Button::new(language.text(TextKey::Retry)),
                    )
                    .clicked();
                save_copy = ui.button(language.text(TextKey::SaveCopy)).clicked();
                discard = ui.button(language.text(TextKey::Discard)).clicked();
            });
        });
        if retry {
            self.retry_soft_locked_save();
        }
        if save_copy {
            self.show_save_copy = true;
        }
        if discard {
            let state = std::mem::replace(&mut self.state, SessionState::Welcome);
            if let SessionState::SoftLocked { session, .. } = state {
                self.state = SessionState::Locked {
                    path: session.path.clone(),
                };
            }
            clear_secret(&mut self.password);
        }
    }

    fn render_error(&mut self, ui: &mut egui::Ui) {
        if let Some(message) = self.transient_message.take() {
            ui.colored_label(Color32::from_rgb(205, 80, 65), message);
        }
        if let Some(error) = &self.error {
            ui.colored_label(
                Color32::from_rgb(205, 80, 65),
                error_text(self.language(), error),
            );
        }
    }

    fn monitor_copy_commands(&self, ctx: &egui::Context) {
        let copied = ctx.output(|output| {
            output.commands.iter().find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = copied {
            let _ = self
                .platform
                .set_clipboard_with_expiry(&text, Duration::from_secs(30));
        }
    }
}

impl eframe::App for SecureNotesApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts_and_close(ctx);
        let now = self.started_at.elapsed();
        self.auto_lock.set_timeout(Duration::from_secs(
            u64::from(self.settings.auto_lock_minutes) * 60,
        ));
        if ctx.input(|input| !input.events.is_empty()) {
            self.auto_lock.record_activity(now);
        }

        let mut session_lock_requested = false;
        while let Ok(event) = self.session_events.try_recv() {
            session_lock_requested |=
                lock_for_session_event(event, self.settings.lock_on_session_events);
        }
        let idle_lock_requested =
            matches!(self.state, SessionState::Unlocked(_)) && self.auto_lock.expired(now);
        if session_lock_requested || idle_lock_requested {
            self.lock_current();
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let locked_path = match &self.state {
            SessionState::Locked { path } => Some(path.clone()),
            _ => None,
        };
        match &self.state {
            SessionState::Welcome => self.render_welcome(ui),
            SessionState::Unlocked(_) => self.render_unlocked(ui),
            SessionState::Locked { .. } => self.render_locked(ui, locked_path.unwrap()),
            SessionState::SoftLocked { .. } => self.render_soft_locked(ui),
        }
        self.render_settings_window(ui.ctx());
        self.render_change_password(ui.ctx());
        self.render_delete_confirmation(ui.ctx());
        self.render_save_copy(ui.ctx());
        self.render_close_prompt(ui.ctx());
        self.monitor_copy_commands(ui.ctx());
    }
}

fn render_empty(ui: &mut egui::Ui, language: Language) {
    ui.vertical_centered(|ui| {
        ui.add_space(120.0);
        ui.heading(language.text(TextKey::EmptyTitle));
        ui.weak(language.text(TextKey::EmptyBody));
    });
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn clear_secret(secret: &mut String) {
    if !secret.is_empty() {
        unsafe {
            libsodium_sys::sodium_memzero(secret.as_mut_ptr().cast(), secret.len());
        }
    }
    secret.clear();
}

fn error_text(language: Language, error: &VaultError) -> &'static str {
    match (language, error) {
        (Language::English, VaultError::UnsupportedVersion(_)) => {
            "This vault version is not supported."
        }
        (Language::English, VaultError::PayloadTooLarge) => {
            "The vault is larger than the 100 MB safety limit."
        }
        (Language::English, VaultError::InvalidPasswordOrKey) => {
            "Incorrect password or damaged key data."
        }
        (Language::English, VaultError::IntegrityViolation) => "The vault integrity check failed.",
        (Language::English, VaultError::MalformedContainer) => {
            "This is not a valid Secure Notes vault."
        }
        (Language::English, VaultError::PasswordTooShort) => "Use at least 8 characters.",
        (Language::English, VaultError::MemoryLockFailed) => {
            "Secure memory could not be locked. The operation was stopped."
        }
        (Language::English, VaultError::CryptoUnavailable) => {
            "Cryptography is unavailable on this system."
        }
        (Language::English, VaultError::FileConflict) => {
            "The vault was changed or removed outside Secure Notes. Save a copy instead."
        }
        (Language::English, VaultError::ReadFailed) => "The vault could not be read.",
        (Language::English, VaultError::WriteFailed) => {
            "The vault could not be saved. The previous file is still valid."
        }
        (Language::Russian, VaultError::UnsupportedVersion(_)) => {
            "Эта версия базы не поддерживается."
        }
        (Language::Russian, VaultError::PayloadTooLarge) => {
            "База превышает безопасный лимит 100 МБ."
        }
        (Language::Russian, VaultError::InvalidPasswordOrKey) => {
            "Неверный пароль или повреждены данные ключа."
        }
        (Language::Russian, VaultError::IntegrityViolation) => {
            "Проверка целостности базы не пройдена."
        }
        (Language::Russian, VaultError::MalformedContainer) => {
            "Файл не является корректной базой Secure Notes."
        }
        (Language::Russian, VaultError::PasswordTooShort) => "Используйте не менее 8 символов.",
        (Language::Russian, VaultError::MemoryLockFailed) => {
            "Не удалось заблокировать защищённую память. Операция остановлена."
        }
        (Language::Russian, VaultError::CryptoUnavailable) => {
            "Криптография недоступна в этой системе."
        }
        (Language::Russian, VaultError::FileConflict) => {
            "База изменена или удалена вне Secure Notes. Сохраните копию."
        }
        (Language::Russian, VaultError::ReadFailed) => "Не удалось прочитать базу.",
        (Language::Russian, VaultError::WriteFailed) => {
            "Не удалось сохранить базу. Предыдущий файл остаётся действительным."
        }
    }
}
