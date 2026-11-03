//! Chips examples: link chips and filter chips over the catalog search.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::SearchParams;

/// A search of the catalog index for `q`.
fn search(q: &str) -> SearchParams {
    SearchParams { q: q.to_string() }
}

pub fn overview(_ctx: &Ctx) -> Node {
    Chips::filter("Search the catalog", crate::catalog_home, |values| search(values.first().copied().unwrap_or("")))
        .option("grid", "Grids")
        .option("card", "Cards")
        .option("form", "Forms")
        .into()
}

pub fn links(_ctx: &Ctx) -> Node {
    Chips::links("Topics")
        .link("Charts", crate::chart, ())
        .link("Tabs", crate::tabs, ())
        .link("Trees", crate::tree, ())
        .into()
}

/// What the catalog shows of `Chips`.
pub const ENTRY: Entry = Entry {
    name: "Chips",
    docs: "components/content#chips",
    api: &["Chips"],
    overview: &[ex!(overview)],
    examples: &[ex!(links)],
};
