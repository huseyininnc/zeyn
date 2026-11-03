//! AutoGrid examples: as many columns as fit the width.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    [
        "Orders", "Stock", "People", "Invoices", "Reports", "Settings",
    ]
    .into_iter()
    .fold(AutoGrid::new().min(160), |tiles, title| {
        tiles.add(Card::new().title(title))
    })
    .into()
}

pub fn wide_columns(_ctx: &Ctx) -> Node {
    AutoGrid::new()
        .min(320)
        .gap(6)
        .add(Panel::new("Left").add(Paragraph::new("At least 320 pixels wide.")))
        .add(Panel::new("Right").add(Paragraph::new("One column on a phone.")))
        .into()
}

/// What the catalog shows of `AutoGrid`.
pub const ENTRY: Entry = Entry {
    name: "AutoGrid",
    docs: "components/content#autogrid",
    api: &["AutoGrid"],
    overview: &[ex!(overview)],
    examples: &[ex!(wide_columns)],
};
