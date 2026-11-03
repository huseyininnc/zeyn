//! MetaLine examples: short facts on one line.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    MetaLine::new().part("12 May 2026").part("5 min read").part("Guides").into()
}

pub fn separator(_ctx: &Ctx) -> Node {
    MetaLine::new().part("Shop").part("Books").part("Atlases").separator("/").into()
}

/// What the catalog shows of `MetaLine`.
pub const ENTRY: Entry = Entry {
    name: "MetaLine",
    docs: "components/content#metaline",
    api: &["MetaLine"],
    overview: &[ex!(overview)],
    examples: &[ex!(separator)],
};
