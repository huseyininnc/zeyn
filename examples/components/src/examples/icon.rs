//! Icon examples: every icon of the set, in toolboxes of twelve with their names as tips.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Icon::ALL
        .chunks(12)
        .fold(Row::new().compact(), |row, icons| {
            row.col(
                icons
                    .iter()
                    .fold(Toolbox::new("Icons"), |bar, icon| {
                        bar.tool(icon.name(), icon.name(), Some(*icon))
                    }),
            )
        })
        .into()
}

/// What the catalog shows of `Icon`.
pub const ENTRY: Entry = Entry {
    name: "Icon",
    docs: "language/screens#toolbar-and-context-menu",
    api: &["Icon"],
    overview: &[ex!(overview)],
    examples: &[],
};
