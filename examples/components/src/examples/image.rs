//! Image examples: pictures with required alt text and size, placeholders, low resolution files
//! and fallbacks.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Image::new(crate::data::picture(5))
        .alt("A soft green gradient with a circle")
        .size(240, 240)
        .into()
}

pub fn progressive(_ctx: &Ctx) -> Node {
    Image::new("/site/photo/7.svg")
        .alt("A gradient with a circle")
        .size(240, 240)
        .lazy()
        .placeholder(Placeholder::color("--z-surface-2"))
        .low("/site/photo/7-low.svg")
        .fit(Fit::Cover)
        .into()
}

pub fn fallback(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Image::new("/site/photo/missing.svg")
                .alt("A photo that is not there")
                .size(160, 160)
                .fallback(crate::data::picture(2)),
        )
        .col(
            Image::new("/site/photo/missing.svg")
                .alt("A photo that is not there")
                .size(160, 160)
                .placeholder(Placeholder::color("#d9dde3")),
        )
        .col(
            Image::new("/site/photo/missing.svg")
                .alt("A photo that is not there")
                .size(160, 160)
                .on_error("No photo yet"),
        )
        .into()
}

pub fn first_image(_ctx: &Ctx) -> Node {
    Image::new(crate::data::picture(9))
        .alt("The first picture a visitor sees")
        .size(480, 240)
        .priority()
        .fit(Fit::Cover)
        .into()
}

/// What the catalog shows of `Image`.
pub const ENTRY: Entry = Entry {
    name: "Image",
    docs: "components/display#image",
    api: &["Image", "Placeholder"],
    overview: &[ex!(overview)],
    examples: &[ex!(progressive), ex!(fallback), ex!(first_image)],
};
