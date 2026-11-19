//! FlowDiagram examples: the steps of a process as boxes joined by arrows.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    FlowDiagram::new("Order process")
        .node("cart", "Cart")
        .node("pay", "Payment")
        .node("pack", "Packing")
        .node("ship", "Shipping")
        .node("done", "Delivered")
        .tone(Tone::Success)
        .edge("cart", "pay")
        .edge("pay", "pack")
        .edge("pack", "ship")
        .edge("ship", "done")
        .into()
}

pub fn branches(_ctx: &Ctx) -> Node {
    FlowDiagram::new("Expense approval")
        .node("sent", "Expense sent")
        .node("lead", "Team lead review")
        .node("finance", "Finance review")
        .node("fix", "Sent back for changes")
        .tone(Tone::Warning)
        .node("paid", "Paid")
        .tone(Tone::Success)
        .node("no", "Rejected")
        .tone(Tone::Danger)
        .edge("sent", "lead")
        .edge_with("lead", "finance", "Approved")
        .edge_with("lead", "fix", "Unclear")
        .edge("fix", "lead")
        .edge_with("finance", "paid", "Approved")
        .edge_with("finance", "no", "Rejected")
        .into()
}

/// What the catalog shows of `FlowDiagram`.
pub const ENTRY: Entry = Entry {
    name: "FlowDiagram",
    docs: "components/data#flowdiagram",
    api: &["FlowDiagram"],
    overview: &[ex!(overview)],
    examples: &[ex!(branches)],
};
