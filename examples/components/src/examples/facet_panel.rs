//! FacetPanel examples: kinds with counts and a price range that filter a card grid.

use crate::data::{Kind, Product, products, search_products};
use crate::{Demo, Entry, exd};
use zeyn::prelude::*;

/// The kinds a facet offers: the value sent, the text and the kind.
const KINDS: [(&str, &str, Kind); 3] = [
    ("Book", "Books", Kind::Book),
    ("Tool", "Tools", Kind::Tool),
    ("Plant", "Plants", Kind::Plant),
];

/// The kinds chosen in the params.
fn chosen(demo: &Demo) -> Vec<Kind> {
    KINDS
        .iter()
        .filter(|(v, _, _)| demo.kinds.iter().any(|k| k == v))
        .map(|(_, _, k)| *k)
        .collect()
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let low = demo.min_price.unwrap_or(0) * 100;
    let high = demo.max_price.unwrap_or(700) * 100;
    let in_price = |p: &&Product| (low..=high).contains(&p.price.minor);
    let kinds = KINDS
        .iter()
        .fold(Facet::new("kinds", "Kind"), |f, (value, text, kind)| {
            let count = products()
                .iter()
                .filter(in_price)
                .filter(|p| p.kind == *kind)
                .count();
            f.option(*value, *text, count as u64)
        })
        .chosen(demo.kinds.clone());
    let panel = FacetPanel::of(crate::facet_panel, demo)
        .facet(kinds)
        .range(
            RangeSlider::new("Price", ["min_price", "max_price"], 0, 700)
                .step(10)
                .money(Currency::Try)
                .value(demo.min_price, demo.max_price),
        )
        .drives("facets");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::kind, chosen(demo))
        .scope_range(
            Product::price,
            demo.min_price
                .map(|m| Money::tl(m * 100)),
            demo.max_price
                .map(|m| Money::tl(m * 100)),
        )
        .image_size(300, 300)
        .paging(Paging::infinite(8))
        .listens("facets");
    Row::new()
        .col(panel)
        .col(cards)
        .weight(3)
        .into()
}

/// What the catalog shows of `FacetPanel`.
pub const ENTRY: Entry = Entry {
    name: "FacetPanel",
    docs: "components/commerce#facet-panel",
    api: &["FacetPanel", "Facet", "CardGrid"],
    overview: &[exd!(overview)],
    examples: &[],
};
