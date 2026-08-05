use uuid::Uuid;

use crate::vault::{Note, Vault};

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
