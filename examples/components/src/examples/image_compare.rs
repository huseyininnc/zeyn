//! ImageCompare examples: two pictures under a slider.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    ImageCompare::new("Compare the two pictures")
        .before(crate::data::picture(2), "The first sample picture")
        .after(crate::data::picture(7), "The second sample picture")
        .size(300, 300)
        .into()
}

/// What the catalog shows of `ImageCompare`.
pub const ENTRY: Entry = Entry {
    name: "ImageCompare",
    docs: "components/display#imagecompare",
    api: &["ImageCompare"],
    overview: &[ex!(overview)],
    examples: &[],
};
