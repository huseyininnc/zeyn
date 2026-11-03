//! Heading examples: section headings from h2 to h6.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(1)
        .add(Heading::h2("Section"))
        .add(Heading::h3("Subsection"))
        .add(Heading::h4("Group"))
        .into()
}

pub fn anchor(_ctx: &Ctx) -> Node {
    Heading::h3("Shipping times")
        .id("shipping")
        .into()
}

/// What the catalog shows of `Heading`.
pub const ENTRY: Entry = Entry {
    name: "Heading",
    docs: "components/content#heading",
    api: &["Heading"],
    overview: &[ex!(overview)],
    examples: &[ex!(anchor)],
};
