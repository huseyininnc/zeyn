//! App header examples: the bar over every page of this catalog, with a search box in its center
//! slot and a link at its end.

use crate::{Entry, Example};
use zeyn::SiteHeader;
use zeyn::prelude::*;

/// The catalog's own app header, set with `App::header(catalog_header)`.
pub fn catalog_header(h: SiteHeader) -> SiteHeader {
    h.center(
        SearchBox::of(crate::find_components)
            .label(t!("cx.search"))
            .suggestion(|f: &crate::Found| f.name.clone())
            .results(crate::catalog_home),
    )
    .end(LinkButton::new(
        t!("cx.all"),
        crate::catalog_home,
        SearchParams::default(),
    ))
}

/// The live part of the example: the bar at the top of this page.
fn this_page(_ctx: &Ctx) -> Node {
    Alert::info("The bar at the top of this page is the app header: try its search box.").into()
}

/// What the catalog shows of the app header.
pub const ENTRY: Entry = Entry {
    name: "App header",
    docs: "components/shell#the-app-header",
    api: &["SiteHeader"],
    overview: &[Example {
        func: "catalog_header",
        build: this_page,
    }],
    examples: &[],
};
