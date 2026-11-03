//! Share examples: the share sheet, or a copied link.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::SearchParams;

pub fn overview(_ctx: &Ctx) -> Node {
    Share::page().into()
}

pub fn another_screen(_ctx: &Ctx) -> Node {
    Share::screen(crate::catalog_home, SearchParams::default())
        .title("Zeyn components")
        .label("Share the catalog")
        .into()
}

/// What the catalog shows of `Share`.
pub const ENTRY: Entry = Entry {
    name: "Share",
    docs: "components/content#share",
    api: &["Share"],
    overview: &[ex!(overview)],
    examples: &[ex!(another_screen)],
};
