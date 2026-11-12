//! Rail examples: a row of cards that scrolls sideways.

use crate::data::{Kind, Product, search_products};
use crate::examples::image_card::{ProductParams, product};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Rail::of(search_products)
        .title("New arrivals")
        .card(|p: &Product| {
            Card::image(p.image.clone(), p.name.clone())
                .text(p.price.to_string())
                .link(product, ProductParams { id: p.id.0 })
        })
        .image_size(300, 300)
        .into()
}

pub fn with_scrollbar(_ctx: &Ctx) -> Node {
    Rail::of(search_products)
        .title("Best sellers")
        .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
        .image_size(300, 300)
        .scrollbar()
        .into()
}

pub fn scoped(_ctx: &Ctx) -> Node {
    Rail::of(search_products)
        .title("Books")
        .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
        .scope(Product::kind, Kind::Book)
        .image_size(300, 300)
        .into()
}

/// What the catalog shows of `Rail`.
pub const ENTRY: Entry = Entry {
    name: "Rail",
    docs: "components/catalog#rail",
    api: &["Rail", "CardGrid"],
    overview: &[ex!(overview)],
    examples: &[ex!(with_scrollbar), ex!(scoped)],
};
