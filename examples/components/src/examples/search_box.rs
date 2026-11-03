//! SearchBox examples: a search field with suggestions.

use crate::Found;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    SearchBox::of(crate::find_components)
        .label("Search components")
        .suggestion(|f: &Found| f.name.clone())
        .results(crate::catalog_home)
        .into()
}

/// What the catalog shows of `SearchBox`.
pub const ENTRY: Entry = Entry {
    name: "SearchBox",
    docs: "components/catalog#search-box",
    api: &["SearchBox"],
    overview: &[ex!(overview)],
    examples: &[],
};
