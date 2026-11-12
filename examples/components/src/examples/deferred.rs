//! Deferred examples: a part that loads after the screen shows.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::Deferred;

pub fn overview(_ctx: &Ctx) -> Node {
    Deferred::new(
        Card::new()
            .title("Sales this year")
            .add(Paragraph::new(
                "This card came in its own request after the page showed.",
            )),
    )
    .lines(3)
    .into()
}

/// What the catalog shows of `Deferred`.
pub const ENTRY: Entry = Entry {
    name: "Deferred",
    docs: "components/layouts",
    api: &["Deferred"],
    overview: &[ex!(overview)],
    examples: &[],
};
