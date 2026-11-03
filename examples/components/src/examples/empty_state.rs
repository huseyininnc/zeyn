//! EmptyState examples: what a list shows when it has nothing.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    EmptyState::new("No orders yet")
        .text("Orders you place show up here.")
        .into()
}

/// What the catalog shows of `EmptyState`.
pub const ENTRY: Entry = Entry {
    name: "EmptyState",
    docs: "components/display#emptystate-and-skeleton",
    api: &["EmptyState"],
    overview: &[ex!(overview)],
    examples: &[],
};
