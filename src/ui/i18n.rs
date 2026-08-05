use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Russian,
    #[default]
    English,
}

impl Language {
    pub fn from_locale(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("ru") {
            Self::Russian
        } else {
            Self::English
        }
    }

    pub fn current_system() -> Self {
        Self::from_locale(&system_locale())
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Russian => "Русский",
            Self::English => "English",
        }
    }

    pub fn text(self, key: TextKey) -> &'static str {
        match (self, key) {
            (Self::English, TextKey::AppTitle) => "Secure Notes",
            (Self::English, TextKey::WelcomeTitle) => "Your notes, sealed locally",
            (Self::English, TextKey::WelcomeBody) => {
                "Open an encrypted vault or create a new one. There is no password recovery."
            }
            (Self::English, TextKey::NewVault) => "Create vault",
            (Self::English, TextKey::OpenVault) => "Open vault",
            (Self::English, TextKey::Create) => "Create",
            (Self::English, TextKey::Open) => "Open",
            (Self::English, TextKey::Password) => "Password",
            (Self::English, TextKey::ConfirmPassword) => "Confirm password",
            (Self::English, TextKey::Unlock) => "Unlock",
            (Self::English, TextKey::Cancel) => "Cancel",
            (Self::English, TextKey::Save) => "Save",
            (Self::English, TextKey::Lock) => "Lock",
            (Self::English, TextKey::Search) => "Search notes…",
            (Self::English, TextKey::NewNote) => "New note",
            (Self::English, TextKey::EmptyTitle) => "No notes yet",
            (Self::English, TextKey::EmptyBody) => "Create a note to start writing.",
            (Self::English, TextKey::NoResults) => "No matching notes",
            (Self::English, TextKey::Delete) => "Delete",
            (Self::English, TextKey::ChangePassword) => "Change password",
            (Self::English, TextKey::Settings) => "Settings",
            (Self::English, TextKey::Modified) => "Unsaved changes",
            (Self::English, TextKey::Saved) => "Saved",
            (Self::English, TextKey::NoteTitle) => "Title",
            (Self::English, TextKey::NoteBody) => "Note",
            (Self::English, TextKey::Untitled) => "Untitled note",
            (Self::English, TextKey::DeleteQuestion) => "Delete this note permanently?",
            (Self::English, TextKey::CloseUnsaved) => "Save changes before closing?",
            (Self::English, TextKey::Discard) => "Discard",
            (Self::English, TextKey::SaveCopy) => "Save a copy",
            (Self::English, TextKey::Retry) => "Retry save",
            (Self::English, TextKey::SoftLockTitle) => "Save needs attention",
            (Self::English, TextKey::SoftLockBody) => {
                "The window is locked, but unsaved data remains in memory. Enter the password to retry, or discard it."
            }
            (Self::English, TextKey::LockedTitle) => "Vault locked",
            (Self::English, TextKey::LockedBody) => "Enter your password to continue.",
            (Self::English, TextKey::AutoLock) => "Lock after inactivity",
            (Self::English, TextKey::Minutes) => "minutes",
            (Self::English, TextKey::LockOnWindows) => "Lock when Windows locks or sleeps",
            (Self::English, TextKey::Language) => "Language",
            (Self::English, TextKey::PasswordMismatch) => "Passwords do not match.",
            (Self::English, TextKey::NewPassword) => "New password",
            (Self::English, TextKey::Change) => "Change",
            (Self::Russian, TextKey::AppTitle) => "Secure Notes",
            (Self::Russian, TextKey::WelcomeTitle) => "Заметки под локальной защитой",
            (Self::Russian, TextKey::WelcomeBody) => {
                "Откройте зашифрованную базу или создайте новую. Восстановления пароля нет."
            }
            (Self::Russian, TextKey::NewVault) => "Создать базу",
            (Self::Russian, TextKey::OpenVault) => "Открыть базу",
            (Self::Russian, TextKey::Create) => "Создать",
            (Self::Russian, TextKey::Open) => "Открыть",
            (Self::Russian, TextKey::Password) => "Пароль",
            (Self::Russian, TextKey::ConfirmPassword) => "Повторите пароль",
            (Self::Russian, TextKey::Unlock) => "Разблокировать",
            (Self::Russian, TextKey::Cancel) => "Отмена",
            (Self::Russian, TextKey::Save) => "Сохранить",
            (Self::Russian, TextKey::Lock) => "Заблокировать",
            (Self::Russian, TextKey::Search) => "Поиск по заметкам…",
            (Self::Russian, TextKey::NewNote) => "Новая заметка",
            (Self::Russian, TextKey::EmptyTitle) => "Заметок пока нет",
            (Self::Russian, TextKey::EmptyBody) => "Создайте заметку и начните писать.",
            (Self::Russian, TextKey::NoResults) => "Ничего не найдено",
            (Self::Russian, TextKey::Delete) => "Удалить",
            (Self::Russian, TextKey::ChangePassword) => "Сменить пароль",
            (Self::Russian, TextKey::Settings) => "Настройки",
            (Self::Russian, TextKey::Modified) => "Есть изменения",
            (Self::Russian, TextKey::Saved) => "Сохранено",
            (Self::Russian, TextKey::NoteTitle) => "Заголовок",
            (Self::Russian, TextKey::NoteBody) => "Заметка",
            (Self::Russian, TextKey::Untitled) => "Без названия",
            (Self::Russian, TextKey::DeleteQuestion) => "Удалить эту заметку безвозвратно?",
            (Self::Russian, TextKey::CloseUnsaved) => "Сохранить изменения перед выходом?",
            (Self::Russian, TextKey::Discard) => "Не сохранять",
            (Self::Russian, TextKey::SaveCopy) => "Сохранить копию",
            (Self::Russian, TextKey::Retry) => "Повторить сохранение",
            (Self::Russian, TextKey::SoftLockTitle) => "Сохранение требует внимания",
            (Self::Russian, TextKey::SoftLockBody) => {
                "Окно заблокировано, но несохранённые данные остаются в памяти. Введите пароль для повтора или удалите изменения."
            }
            (Self::Russian, TextKey::LockedTitle) => "База заблокирована",
            (Self::Russian, TextKey::LockedBody) => "Введите пароль, чтобы продолжить.",
            (Self::Russian, TextKey::AutoLock) => "Блокировать при бездействии",
            (Self::Russian, TextKey::Minutes) => "минут",
            (Self::Russian, TextKey::LockOnWindows) => "Блокировать при блокировке или сне Windows",
            (Self::Russian, TextKey::Language) => "Язык",
            (Self::Russian, TextKey::PasswordMismatch) => "Пароли не совпадают.",
            (Self::Russian, TextKey::NewPassword) => "Новый пароль",
            (Self::Russian, TextKey::Change) => "Сменить",
        }
    }
}

#[derive(Clone, Copy)]
pub enum TextKey {
    AppTitle,
    WelcomeTitle,
    WelcomeBody,
    NewVault,
    OpenVault,
    Create,
    Open,
    Password,
    ConfirmPassword,
    Unlock,
    Cancel,
    Save,
    Lock,
    Search,
    NewNote,
    EmptyTitle,
    EmptyBody,
    NoResults,
    Delete,
    ChangePassword,
    Settings,
    Modified,
    Saved,
    NoteTitle,
    NoteBody,
    Untitled,
    DeleteQuestion,
    CloseUnsaved,
    Discard,
    SaveCopy,
    Retry,
    SoftLockTitle,
    SoftLockBody,
    LockedTitle,
    LockedBody,
    AutoLock,
    Minutes,
    LockOnWindows,
    Language,
    PasswordMismatch,
    NewPassword,
    Change,
}

#[cfg(windows)]
fn system_locale() -> String {
    let mut buffer = [0_u16; 85];
    let length = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buffer) };
    if length <= 1 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize - 1])
}

#[cfg(not(windows))]
fn system_locale() -> String {
    std::env::var("LANG").unwrap_or_default()
}
