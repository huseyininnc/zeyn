//! Tree examples: branches loaded from the server as they open.

use crate::data::product_tree;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Tree::of(product_tree)
        .label("Products by kind")
        .into()
}

/// What the catalog shows of `Tree`.
pub const ENTRY: Entry = Entry {
    name: "Tree",
    docs: "components/data",
    api: &["Tree", "TreeItem"],
    overview: &[ex!(overview)],
    examples: &[],
};
