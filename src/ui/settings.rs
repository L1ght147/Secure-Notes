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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub language: Language,
    pub auto_lock_minutes: u32,
    pub lock_on_session_events: bool,
    pub theme_preference: ThemePreference,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::current_system(),
            auto_lock_minutes: 5,
            lock_on_session_events: true,
            theme_preference: ThemePreference::System,
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
