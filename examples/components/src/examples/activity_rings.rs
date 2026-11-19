//! ActivityRings examples: shares of several goals as rings inside one another.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    ActivityRings::new("Today")
        .ring("Orders packed", 42, 60, Tone::Brand)
        .ring("Calls answered", 18, 20, Tone::Success)
        .ring("Returns handled", 3, 12, Tone::Warning)
        .into()
}

pub fn large(_ctx: &Ctx) -> Node {
    ActivityRings::new("This week")
        .ring("Revenue", 81, 100, Tone::Info)
        .ring("New customers", 34, 40, Tone::Success)
        .ring("Tickets closed", 55, 90, Tone::Warning)
        .ring("Stock counted", 20, 100, Tone::Danger)
        .size(200)
        .into()
}

/// What the catalog shows of `ActivityRings`.
pub const ENTRY: Entry = Entry {
    name: "ActivityRings",
    docs: "components/data#activityrings",
    api: &["ActivityRings"],
    overview: &[ex!(overview)],
    examples: &[ex!(large)],
};
