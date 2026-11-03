//! ImageCard examples: subtitles, badges, progress and a horizontal layout.

use crate::data::{Product, search_products};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    CardGrid::of(search_products)
        .card(|p: &Product| {
            Card::image(p.image.clone(), p.name.clone())
                .subtitle(p.code.clone())
                .text(p.price.to_string())
                .corner(Badge::new("New").tone(Tone::Success))
                .progress(p.stock as f32 / 90.0, "In stock")
        })
        .image_size(300, 300)
        .paging(Paging::infinite(4))
        .into()
}

pub fn horizontal(_ctx: &Ctx) -> Node {
    CardGrid::of(search_products)
        .card(|p: &Product| {
            Card::image(p.image.clone(), p.name.clone())
                .subtitle(p.code.clone())
                .badges([
                    Badge::new("Gift"),
                    Badge::new("Bestseller").tone(Tone::Info),
                ])
                .horizontal()
        })
        .image_size(300, 300)
        .paging(Paging::infinite(4))
        .into()
}

/// What the catalog shows of `ImageCard`.
pub const ENTRY: Entry = Entry {
    name: "ImageCard",
    docs: "components/catalog#card-grid",
    api: &["ImageCard", "Card"],
    overview: &[ex!(overview)],
    examples: &[ex!(horizontal)],
};
