//! Toc examples: links to the headings of the page being shown.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Toc::new("On this page")
                .item("Delivery", "toc-delivery")
                .sub("Abroad", "toc-abroad")
                .item("Returns", "toc-returns"),
        )
        .col(
            Stack::new()
                .add(Heading::h3("Delivery").id("toc-delivery"))
                .add(Paragraph::new(
                    "Orders placed before 14:00 leave the same day.",
                ))
                .add(Heading::h4("Abroad").id("toc-abroad"))
                .add(Paragraph::new(
                    "Parcels abroad take three to seven working days.",
                ))
                .add(Heading::h3("Returns").id("toc-returns"))
                .add(Paragraph::new(
                    "Send it back within 30 days for a full refund.",
                )),
        )
        .weight(3)
        .into()
}

/// What the catalog shows of `Toc`.
pub const ENTRY: Entry = Entry {
    name: "Toc",
    docs: "components/content#toc",
    api: &["Toc"],
    overview: &[ex!(overview)],
    examples: &[],
};
