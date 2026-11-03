//! Paragraph examples: escaped text, normal or quiet.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(1)
        .add(Paragraph::new("Every order ships within two working days."))
        .add(
            Paragraph::new("Prices include tax.")
                .muted()
                .small(),
        )
        .into()
}

/// What the catalog shows of `Paragraph`.
pub const ENTRY: Entry = Entry {
    name: "Paragraph",
    docs: "components/content#paragraph",
    api: &["Paragraph"],
    overview: &[ex!(overview)],
    examples: &[],
};
