use super::models::{Note, NoteId};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;

/// Notes kept in memory. Replace it with a database backed store.
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("note store lock is poisoned".into())
}

impl NoteStore {
    pub fn all(&self) -> Result<Vec<Note>> {
        Ok(self.0.read().map_err(poisoned)?.clone())
    }

    pub fn get(&self, id: NoteId) -> Result<Note> {
        self.0
            .read()
            .map_err(poisoned)?
            .iter()
            .find(|n| n.id == id)
            .cloned()
            .ok_or(Error::NotFound)
    }

    /// Inserts (id 0) or updates; a stale row version fails with `Conflict`.
    pub fn save(&self, mut note: Note) -> Result<Note> {
        let mut rows = self.0.write().map_err(poisoned)?;
        if note.id.0 == 0 {
            note.id = NoteId(
                rows.iter()
                    .map(|n| n.id.0)
                    .max()
                    .unwrap_or(0)
                    + 1,
            );
            note.version = 1;
            rows.push(note.clone());
            return Ok(note);
        }
        let current = rows
            .iter_mut()
            .find(|n| n.id == note.id)
            .ok_or(Error::NotFound)?;
        if current.version != note.version {
            return Err(Error::Conflict);
        }
        note.version += 1;
        *current = note.clone();
        Ok(note)
    }
}
