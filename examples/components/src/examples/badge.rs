//! Badge examples: short status labels in every tone.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Badge::new("Paid").tone(Tone::Success))
        .col(Badge::new("Overdue").tone(Tone::Danger))
        .col(Badge::new("Pending").tone(Tone::Warning))
        .col(Badge::new("New").tone(Tone::Info))
        .col(Badge::new("Draft"))
        .into()
}

/// Status as a dot before plain text, the quiet form for lists and grids, and count pills.
pub fn dots_and_counts(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(
            Badge::new("Active")
                .tone(Tone::Success)
                .dot(),
        )
        .col(
            Badge::new("Paused")
                .tone(Tone::Warning)
                .dot(),
        )
        .col(
            Badge::new("Failed")
                .tone(Tone::Danger)
                .dot(),
        )
        .col(Badge::new("12").count())
        .col(
            Badge::new("3")
                .tone(Tone::Brand)
                .count(),
        )
        .into()
}

/// What the catalog shows of `Badge`.
pub const ENTRY: Entry = Entry {
    name: "Badge",
    docs: "components/display#badge",
    api: &["Badge"],
    overview: &[ex!(overview)],
    examples: &[ex!(dots_and_counts)],
};
