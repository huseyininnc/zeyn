//! DataGrid examples: rows of an action with paging, sorting and filters.

use crate::data::{
    Kind, Product, Warehouse, million_products, pretend_delete, pretend_delete_all,
    pretend_save_product, query_result, search_products, search_warehouses,
};
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::{Aggregate, Value};

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

pub fn confirm_actions(_ctx: &Ctx) -> Node {
    DataGrid::of(search_products)
        .paging(Paging::pages(4))
        .column(Product::code)
        .width(100)
        .column(Product::name)
        .flex(1)
        .row_action("Delete", pretend_delete, |p| p.id)
        .icon(Icon::Trash)
        .confirm("Delete this product? Nothing is stored in the catalog.")
        .tone(Tone::Danger)
        .bulk_action("Delete selected", pretend_delete_all, Product::id)
        .confirm("Delete the selected products?")
        .tone(Tone::Danger)
        .into()
}

pub fn numbers(_ctx: &Ctx) -> Node {
    DataGrid::of(search_warehouses)
        .column(Warehouse::city)
        .flex(1)
        .column(Warehouse::units)
        .width(140)
        .sum()
        .column(Warehouse::tons)
        .width(120)
        .column(Warehouse::opened)
        .width(110)
        .plain()
        .into()
}

pub fn search_and_bands(_ctx: &Ctx) -> Node {
    DataGrid::of(search_products)
        .paging(Paging::pages(5))
        .search()
        .column(Product::code)
        .width(100)
        .pin_start()
        .column(Product::name)
        .flex(1)
        .band("Price and stock", |b| {
            b.column(Product::price)
                .width(130)
                .column(Product::stock)
                .width(110)
        })
        .column(Product::active)
        .width(100)
        .check_icon()
        .into()
}

pub fn add_delete_batch(_ctx: &Ctx) -> Node {
    DataGrid::of(search_products)
        .paging(Paging::pages(5))
        .column(Product::code)
        .width(110)
        .editable()
        .column(Product::name)
        .flex(1)
        .editable()
        .column(Product::price)
        .width(130)
        .editable()
        .column(Product::stock)
        .width(110)
        .editable()
        .edit_with(pretend_save_product)
        .add_with(pretend_save_product)
        .delete_with(pretend_delete_all, Product::id)
        .batch_edit()
        .into()
}

pub fn user_grouping(_ctx: &Ctx) -> Node {
    DataGrid::of(search_products)
        .paging(Paging::pages(5))
        .column(Product::code)
        .width(110)
        .count()
        .filter_builder()
        .column(Product::name)
        .flex(1)
        .filter()
        .column(Product::kind)
        .width(120)
        .filter()
        .groupable()
        .column(Product::active)
        .width(100)
        .groupable()
        .check_icon()
        .column(Product::stock)
        .width(200)
        .sum()
        .summary("Per product", |t| {
            match (
                t.get(Product::stock, Aggregate::Sum),
                t.get(Product::code, Aggregate::Count),
            ) {
                (Some(Value::Int(sum)), Some(Value::Int(n))) if *n > 0 => {
                    Some(Value::Decimal(*sum as f64 / *n as f64))
                }
                _ => None,
            }
        })
        .into()
}

pub fn computed_million(_ctx: &Ctx) -> Node {
    DataGrid::of(million_products)
        .paging(Paging::virtual_scroll())
        .column(Product::id)
        .width(110)
        .no_sort()
        .column(Product::code)
        .width(130)
        .no_sort()
        .column(Product::name)
        .flex(1)
        .no_sort()
        .column(Product::stock)
        .width(110)
        .no_sort()
        .into()
}

pub fn dynamic_columns(_ctx: &Ctx) -> Node {
    DataGrid::dynamic(query_result)
        .paging(Paging::virtual_scroll())
        .into()
}

/// What the catalog shows of `DataGrid`.
pub const ENTRY: Entry = Entry {
    name: "DataGrid",
    docs: "language/datagrid",
    api: &[
        "DataGrid",
        "DataGridColumn",
        "DynamicGrid",
        "DynamicPage",
        "Paging",
    ],
    overview: &[ex!(overview)],
    examples: &[
        ex!(confirm_actions),
        ex!(numbers),
        ex!(search_and_bands),
        ex!(add_delete_batch),
        ex!(user_grouping),
        ex!(computed_million),
        ex!(dynamic_columns),
    ],
};
