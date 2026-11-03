//! Chart examples: bar, donut, combo and gauge charts.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Chart::bar("Sales")
        .labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0])
        .into()
}

pub fn donut(_ctx: &Ctx) -> Node {
    Chart::donut("Products by kind")
        .labels(["Books", "Tools", "Plants"])
        .series("Products", [20.0, 20.0, 20.0])
        .into()
}

pub fn gauge(_ctx: &Ctx) -> Node {
    Chart::gauge("Capacity")
        .value(72.0)
        .range(0.0, 100.0)
        .target(85.0)
        .into()
}

/// What the catalog shows of `Chart`.
pub const ENTRY: Entry = Entry {
    name: "Chart",
    docs: "components/data",
    api: &["Chart"],
    overview: &[ex!(overview)],
    examples: &[ex!(donut), ex!(gauge)],
};
