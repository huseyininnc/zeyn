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

pub fn with_price(_ctx: &Ctx) -> Node {
    CardGrid::of(search_products)
        .card(|p: &Product| {
            let price = Price::new(p.price);
            let price = match p.id.0 % 3 {
                0 => price.was(Money::tl(p.price.minor + p.price.minor / 4)),
                _ => price,
            };
            Card::image(p.image.clone(), p.name.clone())
                .subtitle(p.code.clone())
                .price(price)
        })
        .image_size(300, 300)
        .paging(Paging::infinite(4))
        .into()
}

/// The product of a preview.
#[params]
pub struct ProductParams {
    pub id: i64,
}

/// A small page of one product, opened by the cards of `with_preview` and shown in their
/// previews.
#[screen(route = "/ui/image-card/product/{id}", public)]
pub fn product(p: ProductParams) -> Screen {
    let Some(found) = crate::data::products()
        .iter()
        .find(|x| x.id.0 == p.id)
    else {
        return Screen::new("Product").body(EmptyState::new("No such product"));
    };
    Screen::new(found.name.clone()).body(
        DescriptionList::new()
            .item("Code", found.code.clone())
            .item("Price", found.price.to_string())
            .item("In stock", found.stock.to_string()),
    )
}

pub fn with_preview(_ctx: &Ctx) -> Node {
    Rail::of(search_products)
        .title("Hover a card")
        .card(|p: &Product| {
            Card::image(p.image.clone(), p.name.clone())
                .text(p.price.to_string())
                .link(product, ProductParams { id: p.id.0 })
                .preview(product, ProductParams { id: p.id.0 })
        })
        .image_size(300, 300)
        .into()
}

/// What the catalog shows of `ImageCard`.
pub const ENTRY: Entry = Entry {
    name: "ImageCard",
    docs: "components/catalog#card-grid",
    api: &["ImageCard", "Card"],
    overview: &[ex!(overview)],
    examples: &[ex!(horizontal), ex!(with_price), ex!(with_preview)],
};
