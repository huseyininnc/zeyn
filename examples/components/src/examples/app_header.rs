//! App header examples: the bar over every page of this catalog, with a search box in its center
//! slot and links at its end: all components, the source on GitHub and the project page of the
//! docs.

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
    .end(
        Row::new()
            .compact()
            .col(LinkButton::new(
                t!("cx.all"),
                crate::catalog_home,
                SearchParams::default(),
            ))
            .col(
                ExternalLink::new("https://github.com/huseyininnc/zeyn", t!("cx.github")).new_tab(),
            )
            .col(
                ExternalLink::new("https://docs.zeynsoft.com/project", t!("cx.project")).new_tab(),
            ),
    )
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
        build: |ctx, _| this_page(ctx),
    }],
    examples: &[],
};
