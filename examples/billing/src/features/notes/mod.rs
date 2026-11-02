//! Notes: a complete example feature with a list, an editor and an API.

pub mod actions;
pub mod models;
pub mod screens;
pub mod store;

use zeyn::prelude::*;

pub fn feature() -> Feature {
    Feature::new("notes")
        .route_prefix("/notes")
        .guard(SignedIn)
        .screen(screens::note_list)
        .screen(screens::note_editor)
        .api(actions::search_notes)
        .api(actions::save_note)
        .menu(|m| m.group(t!("notes.menu"), Icon::Edit).item(t!("notes.list"), screens::note_list))
}
