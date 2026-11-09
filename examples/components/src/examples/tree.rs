//! Tree examples: branches loaded from the server as they open.

use crate::data::product_tree;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Tree::of(product_tree)
        .label("Products by kind")
        .into()
}

/// Leaves drag onto a drop target, such as an island; the Insert button or Control+Enter
/// inserts the focused leaf.
pub fn draggable(_ctx: &Ctx) -> Node {
    Tree::of(product_tree)
        .label("Products to place")
        .draggable()
        .into()
}

/// What the catalog shows of `Tree`.
pub const ENTRY: Entry = Entry {
    name: "Tree",
    docs: "components/data",
    api: &["Tree", "TreeItem"],
    overview: &[ex!(overview)],
    examples: &[ex!(draggable)],
};
