//! Pagination examples: links to the pages of a list, with the page in the address.

use crate::{Demo, Entry, exd};
use zeyn::prelude::*;

/// The params of this page with another page number.
fn at(page: u32) -> Demo {
    Demo {
        page: Some(page),
        ..Demo::default()
    }
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let page = demo.page.unwrap_or(1).clamp(1, 12);
    Stack::new()
        .add(Paragraph::new(format!("Articles of page {page} of 12")).muted())
        .add(
            Pagination::new(crate::pagination, at)
                .pages(12)
                .current(page),
        )
        .into()
}

pub fn more_numbers(_ctx: &Ctx, demo: &Demo) -> Node {
    Pagination::new(crate::pagination, at)
        .pages(40)
        .current(demo.page.unwrap_or(1))
        .around(2)
        .into()
}

/// What the catalog shows of `Pagination`.
pub const ENTRY: Entry = Entry {
    name: "Pagination",
    docs: "components/content#pagination",
    api: &["Pagination"],
    overview: &[exd!(overview)],
    examples: &[exd!(more_numbers)],
};
