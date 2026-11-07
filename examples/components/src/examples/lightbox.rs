//! Lightbox examples: images that open full screen, alone or as a set to move through.

use crate::data::picture;
use crate::{Entry, ex};
use zeyn::prelude::*;

/// Captions of the sample pictures.
const CAPTIONS: [&str; 4] = [
    "Morning light over the hills",
    "A quiet street after the rain",
    "The harbor at dusk",
    "Fields in early summer",
];

/// Four pictures in a row: the arrow keys and swipes move between them in the lightbox.
pub fn overview(_ctx: &Ctx) -> Node {
    CAPTIONS
        .iter()
        .zip(1..)
        .fold(AutoGrid::new().min(140), |grid, (caption, n)| {
            grid.add(
                Image::new(picture(n))
                    .alt(*caption)
                    .size(300, 300)
                    .lazy()
                    .placeholder(Placeholder::color("--z-surface-2"))
                    .full(picture(n))
                    .caption(*caption),
            )
        })
        .into()
}

/// One picture without a caption: the lightbox shows it alone.
pub fn single(_ctx: &Ctx) -> Node {
    Image::new(picture(7))
        .alt("A lone tree on a hill")
        .size(300, 300)
        .lightbox()
        .into()
}

/// What the catalog shows of the lightbox.
pub const ENTRY: Entry = Entry {
    name: "Lightbox",
    docs: "components/social#lightbox",
    api: &["Image"],
    overview: &[ex!(overview)],
    examples: &[ex!(single)],
};
