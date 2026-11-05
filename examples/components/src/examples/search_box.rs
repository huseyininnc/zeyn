//! SearchBox examples: a search field with suggestions.

use crate::Found;
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{ListItem, SearchParams};

pub fn overview(_ctx: &Ctx) -> Node {
    SearchBox::of(crate::find_components)
        .label("Search components")
        .suggestion(|f: &Found| f.name.clone())
        .results(crate::catalog_home)
        .into()
}

pub fn rich(_ctx: &Ctx) -> Node {
    SearchBox::of(crate::data::search_products)
        .label("Search products")
        .item(|p: &crate::data::Product| {
            ListItem::new(p.name.clone())
                .subtitle(p.code.clone())
                .image(p.image.clone())
                .link(crate::catalog_home, SearchParams { q: p.name.clone() })
        })
        .results(crate::catalog_home)
        .into()
}

/// What the catalog shows of `SearchBox`.
pub const ENTRY: Entry = Entry {
    name: "SearchBox",
    docs: "components/catalog#search-box",
    api: &["SearchBox"],
    overview: &[ex!(overview)],
    examples: &[ex!(rich)],
};
