//! Tabs examples: panels behind a tab list, loaded the first time they open.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Tabs::new("Product")
        .tab(
            "Description",
            Paragraph::new("A sturdy notebook with 120 pages."),
        )
        .tab("Shipping", Paragraph::new("Ships in two days."))
        .tab("Reviews", Paragraph::new("No reviews yet."))
        .into()
}

/// What the catalog shows of `Tabs`.
pub const ENTRY: Entry = Entry {
    name: "Tabs",
    docs: "components/layouts#tabs",
    api: &["Tabs"],
    overview: &[ex!(overview)],
    examples: &[],
};
