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

/// The tab opened last in the remembered example, kept for the session unless the visitor
/// consented to preferences.
pub const DELIVERY_TAB: zeyn::ClientPref<zeyn::Choice> = zeyn::ClientPref::choice(
    "catalog-tabs",
    std::time::Duration::from_secs(30 * 24 * 60 * 60),
    zeyn::Purpose::Preference,
);

pub fn remembered(_ctx: &Ctx) -> Node {
    Tabs::new("Delivery")
        .tab("Standard", Paragraph::new("Three to five days."))
        .tab("Express", Paragraph::new("Next day before noon."))
        .tab("Pick up", Paragraph::new("Ready in two hours."))
        .remember(DELIVERY_TAB)
        .into()
}

/// What the catalog shows of `Tabs`.
pub const ENTRY: Entry = Entry {
    name: "Tabs",
    docs: "components/layouts#tabs",
    api: &["Tabs"],
    overview: &[ex!(overview)],
    examples: &[ex!(remembered)],
};
