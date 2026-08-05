use std::time::Duration;

use uuid::Uuid;

use crate::platform::SessionEvent;
use crate::vault::{Note, Vault};

pub struct AutoLockTimer {
    last_activity: Duration,
    timeout: Duration,
}

impl AutoLockTimer {
    pub fn new(timeout: Duration) -> Self {
        Self {
            last_activity: Duration::ZERO,
            timeout,
        }
    }

    pub fn record_activity(&mut self, now: Duration) {
        self.last_activity = now;
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn expired(&self, now: Duration) -> bool {
        now.saturating_sub(self.last_activity) >= self.timeout
    }
}

pub fn lock_for_session_event(event: SessionEvent, enabled: bool) -> bool {
    enabled && matches!(event, SessionEvent::Locked | SessionEvent::Sleeping)
}

pub fn close_requires_confirmation(dirty: bool, soft_locked: bool) -> bool {
    dirty || soft_locked
}

pub fn create_note(vault: &mut Vault, modified_at: i64) -> Uuid {
    let id = Uuid::new_v4();
    vault.notes.insert(
        0,
        Note {
            id,
            title: String::new(),
            body: String::new(),
            modified_at,
        },
    );
    id
}

#[cfg(test)]
pub fn rename_note(
    vault: &mut Vault,
    id: Uuid,
    title: impl Into<String>,
    modified_at: i64,
) -> Option<()> {
    let note = vault.notes.iter_mut().find(|note| note.id == id)?;
    note.title = title.into();
    note.modified_at = modified_at;
    Some(())
}

pub fn delete_note(vault: &mut Vault, id: Uuid) -> bool {
    let original_len = vault.notes.len();
    vault.notes.retain(|note| note.id != id);
    vault.notes.len() != original_len
}

pub fn search_notes(vault: &Vault, query: &str) -> Vec<Uuid> {
    let query = query.trim().to_lowercase();
    vault
        .notes
        .iter()
        .filter(|note| {
            query.is_empty()
                || note.title.to_lowercase().contains(&query)
                || note.body.to_lowercase().contains(&query)
        })
        .map(|note| note.id)
        .collect()
}
