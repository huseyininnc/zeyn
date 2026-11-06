//! Screen header examples: a title with a subtitle and the main actions at the end of its row.

use crate::{Entry, Example};
use zeyn::prelude::*;

/// Which orders the page shows. Every field is optional, so the page also opens from a plain
/// address or a menu link without params.
#[params]
pub struct OrdersFilter {
    pub status: Option<String>,
}

/// A page with a header: the subtitle under the title, the main action on the right, the other
/// tools under them. On a phone the action wraps under the title.
#[screen(route = "/ui/screen-header/orders", public)]
pub fn orders_page(p: OrdersFilter) -> Screen {
    let shown = match p.status.as_deref() {
        Some("open") => "Open orders of the last 90 days, newest first.",
        _ => "Every order of the last 90 days, newest first.",
    };
    Screen::new("Orders")
        .subtitle(shown)
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
    let open = OrdersFilter {
        status: Some("open".into()),
    };
    Card::new()
        .add(Paragraph::new(
            "The header is part of the page, so the example opens as one.",
        ))
        .add(
            Row::new()
                .compact()
                .col(LinkButton::new(
                    "Open the orders page",
                    orders_page,
                    OrdersFilter { status: None },
                ))
                .col(LinkButton::new("Only open orders", orders_page, open)),
        )
        .into()
}

/// What the catalog shows of the screen header.
pub const ENTRY: Entry = Entry {
    name: "Screen header",
    docs: "language/screens#header",
    api: &["Screen"],
    overview: &[Example {
        func: "orders_page",
        build: |ctx, _| open_orders(ctx),
    }],
    examples: &[],
};
