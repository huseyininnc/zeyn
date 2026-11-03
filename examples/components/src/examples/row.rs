//! Row examples: columns side by side that stack on narrow screens.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(Card::new().title("Orders"))
        .col(Card::new().title("Stock"))
        .col(Card::new().title("People"))
        .into()
}

pub fn weights(_ctx: &Ctx) -> Node {
    Row::new()
        .col(Card::new().title("Weight 2"))
        .weight(2)
        .col(Card::new().title("Weight 1"))
        .into()
}

pub fn compact(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Badge::new("Paid").tone(Tone::Success))
        .col(Badge::new("Overdue").tone(Tone::Danger))
        .col(Badge::new("Draft"))
        .into()
}

/// What the catalog shows of `Row`.
pub const ENTRY: Entry = Entry {
    name: "Row",
    docs: "components/layouts#row",
    api: &["Row"],
    overview: &[ex!(overview)],
    examples: &[ex!(weights), ex!(compact)],
};
