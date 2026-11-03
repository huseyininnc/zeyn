//! Skeleton examples: placeholder lines while content loads.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Skeleton::lines(3).into()
}

/// What the catalog shows of `Skeleton`.
pub const ENTRY: Entry = Entry {
    name: "Skeleton",
    docs: "components/display#emptystate-and-skeleton",
    api: &["Skeleton"],
    overview: &[ex!(overview)],
    examples: &[],
};
