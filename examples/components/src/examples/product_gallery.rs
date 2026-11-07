//! ProductGallery examples: photos with thumbnails, a low resolution first look and a full
//! screen view.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    (1..=6)
        .fold(ProductGallery::new("Linen shirt"), |g, n| {
            g.image(
                Image::new(format!("/site/photo/{n}.svg"))
                    .alt(format!("Linen shirt, view {n}"))
                    .size(600, 600)
                    .low(format!("/site/photo/{n}-low.svg"))
                    .placeholder(Placeholder::color("--z-surface-2")),
            )
            .thumb(format!("/site/photo/{n}-low.svg"))
        })
        .into()
}

pub fn dots(_ctx: &Ctx) -> Node {
    (1..=4)
        .fold(ProductGallery::new("Plant pot"), |g, n| {
            g.image(
                Image::new(crate::data::picture(n))
                    .alt(format!("Plant pot, view {n}"))
                    .size(400, 400),
            )
        })
        .into()
}

/// What the catalog shows of `ProductGallery`.
pub const ENTRY: Entry = Entry {
    name: "ProductGallery",
    docs: "components/commerce#product-gallery",
    api: &["ProductGallery"],
    overview: &[ex!(overview)],
    examples: &[ex!(dots)],
};
