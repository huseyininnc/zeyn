//! CopyButton examples: a button that copies a text, shown or not.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    CopyButton::new("1Z999AA10123456784")
        .shown()
        .into()
}

pub fn own_label(_ctx: &Ctx) -> Node {
    CopyButton::new("https://example.com/join/k3v9")
        .label("Copy the invitation link")
        .into()
}

/// What the catalog shows of `CopyButton`.
pub const ENTRY: Entry = Entry {
    name: "CopyButton",
    docs: "components/content#copybutton",
    api: &["CopyButton"],
    overview: &[ex!(overview)],
    examples: &[ex!(own_label)],
};
