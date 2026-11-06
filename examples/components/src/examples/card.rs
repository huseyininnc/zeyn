//! Card examples: a bordered box with an optional title.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Card::new()
        .title("Shipping")
        .add(Paragraph::new(
            "Orders placed before noon leave the same day.",
        ))
        .into()
}

pub fn nested(_ctx: &Ctx) -> Node {
    Card::new()
        .title("Summary")
        .add(
            Row::new()
                .col(Kpi::new("Orders", 128.0))
                .col(Kpi::new("Returns", 3.0)),
        )
        .into()
}

/// A title row action and a footer; cards in one row take the height of the tallest.
pub fn action_and_footer(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Card::new()
                .title("Open orders")
                .action(Badge::new("12").count())
                .add(Paragraph::new("Twelve orders wait for packing."))
                .footer(Paragraph::new("Updated a minute ago.")),
        )
        .col(
            Card::new()
                .title("Returns")
                .add(Paragraph::new("No returns today.")),
        )
        .into()
}

/// What the catalog shows of `Card`.
pub const ENTRY: Entry = Entry {
    name: "Card",
    docs: "components/layouts#card-and-panel",
    api: &["Card"],
    overview: &[ex!(overview)],
    examples: &[ex!(nested), ex!(action_and_footer)],
};
