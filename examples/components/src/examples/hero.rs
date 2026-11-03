//! Hero examples: a large image section, alone or with slides.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Hero::new("Fresh every morning")
        .text("Order before nine and it is at your door by noon.")
        .image(crate::data::picture(1))
        .into()
}

pub fn slides(_ctx: &Ctx) -> Node {
    Hero::new("Spring collection")
        .text("Pick a slide below.")
        .image(crate::data::picture(2))
        .slide("Twelve new teas")
        .text("From four countries, in small tins.")
        .image(crate::data::picture(4))
        .slide("Free delivery")
        .text("On every order over 500.")
        .into()
}

pub fn rotating(_ctx: &Ctx) -> Node {
    Hero::new("Rotates every eight seconds")
        .text("Hover, focus or the pause button stops it.")
        .image(crate::data::picture(5))
        .slide("Second slide")
        .image(crate::data::picture(6))
        .rotate(8)
        .into()
}

/// What the catalog shows of `Hero`.
pub const ENTRY: Entry = Entry {
    name: "Hero",
    docs: "components/content#hero",
    api: &["Hero"],
    overview: &[ex!(overview)],
    examples: &[ex!(slides), ex!(rotating)],
};
