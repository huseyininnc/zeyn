//! Spoiler examples: content hidden until opened.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Spoiler::new("Show the answer").add(Paragraph::new("Water at about 80 degrees and two minutes of steeping.")).into()
}

/// What the catalog shows of `Spoiler`.
pub const ENTRY: Entry = Entry {
    name: "Spoiler",
    docs: "components/content#spoiler",
    api: &["Spoiler"],
    overview: &[ex!(overview)],
    examples: &[],
};
