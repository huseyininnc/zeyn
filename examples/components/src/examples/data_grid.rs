//! DataGrid examples: rows of an action with paging, sorting and filters.

use crate::data::{Kind, Product, search_products};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    DataGrid::of(search_products)
        .paging(Paging::pages(6))
        .column(Product::code)
        .width(100)
        .column(Product::name)
        .flex(1)
        .filter()
        .column(Product::kind)
        .width(110)
        .badge(|k| match k {
            Kind::Book => Tone::Info,
            Kind::Tool => Tone::Neutral,
            Kind::Plant => Tone::Success,
        })
        .column(Product::price)
        .width(130)
        .column(Product::stock)
        .width(130)
        .progress(90)
        .into()
}

/// What the catalog shows of `DataGrid`.
pub const ENTRY: Entry = Entry {
    name: "DataGrid",
    docs: "language/datagrid",
    api: &["DataGrid", "DataGridColumn", "Paging"],
    overview: &[ex!(overview)],
    examples: &[],
};
