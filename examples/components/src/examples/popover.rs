//! Popover examples: a button that opens a small panel.

use crate::data::{Product, products};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    let p = &products()[1];
    Popover::new("Details")
        .title("Product")
        .add(
            DescriptionList::of(p)
                .field(Product::code)
                .field(Product::price)
                .field(Product::stock),
        )
        .into()
}

/// What the catalog shows of `Popover`.
pub const ENTRY: Entry = Entry {
    name: "Popover",
    docs: "components/display#tooltip-popover-and-busy-overlay",
    api: &["Popover"],
    overview: &[ex!(overview)],
    examples: &[],
};
