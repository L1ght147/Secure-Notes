use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::i18n::Language;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteSortOrder {
    #[default]
    ModifiedNewestFirst,
    TitleAscending,
}

impl NoteSortOrder {
    pub fn short_label(self, language: Language) -> &'static str {
        match (language, self) {
            (Language::Russian, Self::ModifiedNewestFirst) => "Новые",
            (Language::Russian, Self::TitleAscending) => "А–Я",
            (Language::English, Self::ModifiedNewestFirst) => "Newest",
            (Language::English, Self::TitleAscending) => "A–Z",
        }
    }

    pub fn label(self, language: Language) -> &'static str {
        match (language, self) {
            (Language::Russian, Self::ModifiedNewestFirst) => "Новые сверху",
            (Language::Russian, Self::TitleAscending) => "Название А–Я",
            (Language::English, Self::ModifiedNewestFirst) => "Newest first",
            (Language::English, Self::TitleAscending) => "Title A–Z",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub language: Language,
    pub auto_lock_minutes: u32,
    pub lock_on_session_events: bool,
    pub theme_preference: ThemePreference,
    pub note_sort_order: NoteSortOrder,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::current_system(),
            auto_lock_minutes: 5,
            lock_on_session_events: true,
            theme_preference: ThemePreference::System,
            note_sort_order: NoteSortOrder::ModifiedNewestFirst,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = settings_path() else {
            return;
        };
        let Ok(bytes) = serde_json::to_vec_pretty(self) else {
            return;
        };
        let _ = fs::write(path, bytes);
    }
}

fn settings_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|directory| directory.join("secure-notes-settings.json"))
}
