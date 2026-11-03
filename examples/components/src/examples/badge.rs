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

/// What the catalog shows of `Badge`.
pub const ENTRY: Entry = Entry {
    name: "Badge",
    docs: "components/display#badge",
    api: &["Badge"],
    overview: &[ex!(overview)],
    examples: &[],
};
