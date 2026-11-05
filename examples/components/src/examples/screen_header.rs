//! Screen header examples: a title with a subtitle and the main actions at the end of its row.

use crate::{Entry, Example};
use zeyn::prelude::*;

/// A page with a header: the subtitle under the title, the main action on the right, the other
/// tools under them. On a phone the action wraps under the title.
#[screen(route = "/ui/screen-header/orders", public)]
pub fn orders_page() -> Screen {
    Screen::new("Orders")
        .subtitle("Every order of the last 90 days, newest first.")
        .primary(|t| {
            t.button("New order")
                .icon(Icon::Plus)
                .shortcut("Alt+N")
        })
        .toolbar(|t| t.export(Export::Csv))
        .body(Paragraph::new("The list of orders goes here."))
}

/// The live part of the example: the page above, opened in place.
fn open_orders(_ctx: &Ctx) -> Node {
    Card::new()
        .add(Paragraph::new(
            "The header is part of the page, so the example opens as one.",
        ))
        .add(LinkButton::new("Open the orders page", orders_page, ()))
        .into()
}

/// What the catalog shows of the screen header.
pub const ENTRY: Entry = Entry {
    name: "Screen header",
    docs: "language/screens#header",
    api: &["Screen"],
    overview: &[Example {
        func: "orders_page",
        build: open_orders,
    }],
    examples: &[],
};
