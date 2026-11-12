//! Funnel examples: stages that narrow from the first to the last.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Funnel::new("Sales funnel")
        .stage("Visits", 12_400)
        .stage("Product views", 7_900)
        .stage("Carts", 3_100)
        .stage("Orders", 870)
        .into()
}

pub fn another_tone(_ctx: &Ctx) -> Node {
    Funnel::new("Hiring")
        .stage("Applications", 420)
        .stage("Interviews", 64)
        .stage("Offers", 9)
        .tone(Tone::Success)
        .into()
}

/// What the catalog shows of `Funnel`.
pub const ENTRY: Entry = Entry {
    name: "Funnel",
    docs: "components/data#funnel",
    api: &["Funnel"],
    overview: &[ex!(overview)],
    examples: &[ex!(another_tone)],
};
