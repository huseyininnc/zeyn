//! Image examples: pictures with required alt text and size.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Image::new(crate::data::picture(5))
        .alt("A soft green gradient with a circle")
        .size(240, 240)
        .into()
}

/// What the catalog shows of `Image`.
pub const ENTRY: Entry = Entry {
    name: "Image",
    docs: "components/display#image",
    api: &["Image"],
    overview: &[ex!(overview)],
    examples: &[],
};
