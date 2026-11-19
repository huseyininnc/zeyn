//! FileDrop examples: a drop zone outside a form that uploads into the memory store of the chat
//! examples, which keeps the last `MAX_SAMPLE_FILES`.

use crate::examples::composer::CHAT_FILES;
use crate::{Entry, ex};
use zeyn::prelude::*;

/// Takes each stored file of the example; the catalog keeps nothing more than the store does.
#[action(public)]
pub async fn add_sample(_file: File) -> Result<()> {
    Ok(())
}

pub fn overview(_ctx: &Ctx) -> Node {
    FileDrop::new("Documents", CHAT_FILES)
        .on_upload(add_sample)
        .into()
}

pub fn single(_ctx: &Ctx) -> Node {
    FileDrop::new("Profile photo", CHAT_FILES)
        .single()
        .into()
}

/// What the catalog shows of `FileDrop`.
pub const ENTRY: Entry = Entry {
    name: "FileDrop",
    docs: "language/forms#file-drop",
    api: &["FileDrop"],
    overview: &[ex!(overview)],
    examples: &[ex!(single)],
};
