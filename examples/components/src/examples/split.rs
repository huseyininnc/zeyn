//! Split examples: two panes with a handle between them.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Split::new(
        Card::new()
            .title("List")
            .add(Paragraph::new("Drag the handle or use the arrow keys.")),
        Card::new().title("Details"),
    )
    .into()
}

/// What the catalog shows of `Split`.
pub const ENTRY: Entry = Entry {
    name: "Split",
    docs: "components/layouts#split",
    api: &["Split"],
    overview: &[ex!(overview)],
    examples: &[],
};
