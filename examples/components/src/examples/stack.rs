//! Stack examples: nodes one under the other.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(Alert::info("First"))
        .add(Alert::success("Second"))
        .add(Alert::warning("Third"))
        .into()
}

pub fn gap(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Stack::new()
                .gap(0)
                .add(Badge::new("Gap 0"))
                .add(Badge::new("Gap 0")),
        )
        .col(
            Stack::new()
                .gap(6)
                .add(Badge::new("Gap 6"))
                .add(Badge::new("Gap 6")),
        )
        .into()
}

/// What the catalog shows of `Stack`.
pub const ENTRY: Entry = Entry {
    name: "Stack",
    docs: "components/layouts#stack",
    api: &["Stack"],
    overview: &[ex!(overview)],
    examples: &[ex!(gap)],
};
