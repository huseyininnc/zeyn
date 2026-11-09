//! CodeView and JsonView examples: highlighted, numbered and folded on the server.

use crate::{Entry, ex};
use zeyn::prelude::*;

const QUERY: &str = "-- Open orders of the last week, newest first\nSELECT o.number, c.name, o.total\nFROM orders o\nJOIN customers c ON c.id = o.customer_id\nWHERE o.state = 'open'\n  AND o.created_at > now() - interval '7 days'\nORDER BY o.created_at DESC\nLIMIT 50;";

const RUST: &str = "#[action(permission = \"sales.order.read\")]\npub async fn search_orders(q: DataQuery, store: Orders) -> Result<Page<Order>> {\n    let rows = store.list()?;\n    if rows.is_empty() {\n        return Ok(Page::empty());\n    }\n    Ok(q.apply(rows))\n}";

pub fn overview(_ctx: &Ctx) -> Node {
    CodeView::new(CodeLang::Sql, QUERY)
        .title("open_orders.sql")
        .into()
}

pub fn rust(_ctx: &Ctx) -> Node {
    CodeView::new(CodeLang::Rust, RUST)
        .title("orders.rs")
        .into()
}

pub fn json(_ctx: &Ctx) -> Node {
    let payload = serde_json::json!({
        "event": "order.paid",
        "order": { "number": "SO-1042", "total": 129.9, "lines": [
            { "sku": "TEA-01", "qty": 2 },
            { "sku": "CUP-07", "qty": 1 }
        ] },
        "test": false
    });
    JsonView::new(&payload)
        .title("Last webhook payload")
        .into()
}

/// What the catalog shows of `CodeView`.
pub const ENTRY: Entry = Entry {
    name: "CodeView",
    docs: "components/admin#codeview-and-jsonview",
    api: &["CodeView", "JsonView"],
    overview: &[ex!(overview)],
    examples: &[ex!(rust), ex!(json)],
};
