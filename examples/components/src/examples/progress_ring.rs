//! ProgressRing examples: a share of a whole as a ring.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(ProgressRing::new("Import", 64, 100))
        .col(ProgressRing::new("Storage", 92, 100).tone(Tone::Warning))
        .col(ProgressRing::new("Tasks done", 18, 18).tone(Tone::Success))
        .into()
}

pub fn sizes(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(ProgressRing::new("Small", 40, 100).size(40))
        .col(ProgressRing::new("Regular", 40, 100))
        .col(
            ProgressRing::new("Large", 40, 100)
                .size(120)
                .tone(Tone::Brand),
        )
        .into()
}

/// What the catalog shows of `ProgressRing`.
pub const ENTRY: Entry = Entry {
    name: "ProgressRing",
    docs: "components/display#progressring",
    api: &["ProgressRing"],
    overview: &[ex!(overview)],
    examples: &[ex!(sizes)],
};
