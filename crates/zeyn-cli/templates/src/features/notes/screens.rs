use super::actions::{save_note, search_notes};
use super::models::{Note, NoteId};
use super::services::NoteStore;
use zeyn::prelude::*;

/// Which note the editor opens; `None` creates one.
#[params]
pub struct NoteParams {
    pub id: Option<NoteId>,
}

/// List of notes.
#[screen(route = "/", permission = "notes.note.read")]
pub fn note_list() -> Screen {
    Screen::new(t!("notes.list"))
        .toolbar(|t| {
            t.button(t!("notes.new"))
                .icon(Icon::Plus)
                .shortcut("Alt+N")
                .opens(note_editor, NoteParams { id: None })
        })
        .body(
            DataGrid::auto(search_notes)
                .mode(DataMode::Server)
                .on_open(|n| Window::open(note_editor, NoteParams { id: Some(n.id) })),
        )
}

/// Note editor window.
#[screen(permission = "notes.note.write")]
pub fn note_editor(params: NoteParams, store: NoteStore) -> Result<Screen> {
    let note = match params.id {
        Some(id) => store.get(id)?,
        None => Note::default(),
    };
    Ok(Screen::window(t!("notes.editor")).body(
        Form::auto(note)
            .submit(t!("notes.save"), save_note)
            .on_success(|_, ui| {
                ui.toast(t!("notes.saved"));
                ui.close_and_refresh();
            }),
    ))
}
