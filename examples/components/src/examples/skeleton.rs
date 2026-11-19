//! Skeleton examples: placeholders in the shape of the content that loads.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Skeleton::lines(3).into()
}

pub fn shapes(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(
            Row::new()
                .col(Skeleton::kpi())
                .col(Skeleton::kpi()),
        )
        .add(Skeleton::list(3))
        .add(Skeleton::chart())
        .add(Skeleton::grid(4))
        .into()
}

/// What the catalog shows of `Skeleton`.
pub const ENTRY: Entry = Entry {
    name: "Skeleton",
    docs: "components/display#emptystate-and-skeleton",
    api: &["Skeleton"],
    overview: &[ex!(overview)],
    examples: &[ex!(shapes)],
};
