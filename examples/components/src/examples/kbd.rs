//! Kbd examples: keys to press, one or a combination.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Paragraph::new("Search every page with"))
        .col(Kbd::new("Alt+Shift+P"))
        .into()
}

pub fn single_keys(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Kbd::new("Esc"))
        .col(Kbd::new("Enter"))
        .col(Kbd::new("F2"))
        .into()
}

pub fn plus_key(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Paragraph::new("Zoom in with"))
        .col(Kbd::keys(["Ctrl", "+"]))
        .into()
}

/// What the catalog shows of `Kbd`.
pub const ENTRY: Entry = Entry {
    name: "Kbd",
    docs: "components/content#kbd",
    api: &["Kbd"],
    overview: &[ex!(overview)],
    examples: &[ex!(single_keys), ex!(plus_key)],
};
