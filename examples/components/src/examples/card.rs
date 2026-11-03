//! Card examples: a bordered box with an optional title.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Card::new().title("Shipping").add(Paragraph::new("Orders placed before noon leave the same day.")).into()
}

pub fn nested(_ctx: &Ctx) -> Node {
    Card::new().title("Summary").add(Row::new().col(Kpi::new("Orders", 128.0)).col(Kpi::new("Returns", 3.0))).into()
}

/// What the catalog shows of `Card`.
pub const ENTRY: Entry = Entry {
    name: "Card",
    docs: "components/layouts#card-and-panel",
    api: &["Card"],
    overview: &[ex!(overview)],
    examples: &[ex!(nested)],
};
