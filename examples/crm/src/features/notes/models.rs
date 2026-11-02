use zeyn::prelude::*;

/// A note with an optional due date.
#[entity(table = "notes")]
pub struct Note {
    #[key]
    pub id: NoteId,
    #[rule(required, max_len = 120)]
    pub title: String,
    #[rule(max_len = 2000)]
    pub body: Option<String>,
    pub due: Option<Date>,
    pub done: bool,
    #[version]
    pub version: i64,
}
