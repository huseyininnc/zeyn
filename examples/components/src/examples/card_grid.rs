//! CardGrid examples: rows of an action as cards, and a large product catalog that loads fast.

use crate::data::{Item, Product, SHELVES, search_items, search_products};
use crate::examples::image_card::{ProductParams, product};
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

/// One shelf of the large catalog: a placeholder color, a low resolution photo first, a
/// fallback photo, and the first cards of the first shelf loaded before the rest.
fn shelf(n: i64) -> CardGrid<Item> {
    CardGrid::of(search_items)
        .card(|i: &Item| {
            Card::image(i.photo(), i.name.clone())
                .text(i.price.to_string())
                .low(i.low())
                .fallback(crate::data::picture(1))
                .link(product, ProductParams { id: i.id.0 })
        })
        .placeholder(Placeholder::color("--z-surface-2"))
        .priority(if n == 0 { 4 } else { 0 })
        .scope(Item::shelf, n)
        .image_size(300, 300)
        .paging(Paging::infinite(500))
}

pub fn fast_catalog(_ctx: &Ctx) -> Node {
    shelf(0)
        .paging(Paging::infinite(8))
        .into()
}

/// Every item of the large catalog, 2,000 cards on one page, to measure loading and scrolling.
#[screen(route = "/ui/card-grid/catalog", public)]
pub fn large_catalog() -> Screen {
    (0..SHELVES).fold(Screen::new("2,000 products"), |s, n| s.body(shelf(n)))
}

/// What the catalog shows of `CardGrid`.
pub const ENTRY: Entry = Entry {
    name: "CardGrid",
    docs: "components/catalog#card-grid",
    api: &["CardGrid"],
    overview: &[ex!(overview)],
    examples: &[ex!(pages), ex!(fast_catalog)],
};
