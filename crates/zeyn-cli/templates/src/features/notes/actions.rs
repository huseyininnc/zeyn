use super::models::Note;
use super::store::NoteStore;
use zeyn::prelude::*;

/// Notes for the list and the API.
#[action(permission = "notes.note.read")]
pub async fn search_notes(q: GridQuery, store: NoteStore) -> Result<Page<Note>> {
    Ok(q.apply(store.all()?))
}

/// Creates or updates a note.
#[action(permission = "notes.note.write", audit)]
pub async fn save_note(note: Valid<Note>, store: NoteStore) -> Result<Note> {
    store.save(note.into_inner())
}
