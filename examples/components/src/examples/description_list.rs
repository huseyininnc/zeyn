//! DescriptionList examples: label and value pairs of a record.

use crate::data::{Product, products};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    let p = &products()[0];
    DescriptionList::of(p)
        .field(Product::code)
        .field(Product::name)
        .field(Product::kind)
        .field(Product::price)
        .columns(2)
        .into()
}

/// What the catalog shows of `DescriptionList`.
pub const ENTRY: Entry = Entry {
    name: "DescriptionList",
    docs: "components/display#descriptionlist",
    api: &["DescriptionList", "DescriptionOf"],
    overview: &[ex!(overview)],
    examples: &[],
};
