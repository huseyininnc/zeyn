//! CardGrid examples: rows of an action as cards.

use crate::data::{Product, search_products};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    CardGrid::of(search_products)
        .title("Products")
        .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()).text(p.price.to_string()))
        .sort_option("Name", Product::name, false)
        .sort_option("Cheapest", Product::price, false)
        .image_size(300, 300)
        .paging(Paging::infinite(8))
        .into()
}

pub fn pages(_ctx: &Ctx) -> Node {
    CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
        .image_size(300, 300)
        .paging(Paging::pages(4))
        .into()
}

/// What the catalog shows of `CardGrid`.
pub const ENTRY: Entry = Entry {
    name: "CardGrid",
    docs: "components/catalog#card-grid",
    api: &["CardGrid"],
    overview: &[ex!(overview)],
    examples: &[ex!(pages)],
};
