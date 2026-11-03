//! PrevNext examples: links to the previous and the next item.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    PrevNext::new()
        .prev(crate::breadcrumb, ())
        .prev_title("Breadcrumb")
        .next(crate::alpha_index, ())
        .next_title("AlphaIndex")
        .into()
}

pub fn one_side(_ctx: &Ctx) -> Node {
    PrevNext::new()
        .next(crate::alpha_index, ())
        .into()
}

/// What the catalog shows of `PrevNext`.
pub const ENTRY: Entry = Entry {
    name: "PrevNext",
    docs: "components/content#prevnext",
    api: &["PrevNext"],
    overview: &[ex!(overview)],
    examples: &[ex!(one_side)],
};
