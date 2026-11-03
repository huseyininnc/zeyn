//! Alert examples: messages in every tone.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(2)
        .add(Alert::info("A new version is available."))
        .add(Alert::warning("Three products are low on stock.").title("Stock"))
        .into()
}

pub fn tones(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(2)
        .add(Alert::success("Saved."))
        .add(Alert::danger("The file could not be read.").dismissible())
        .into()
}

/// What the catalog shows of `Alert`.
pub const ENTRY: Entry = Entry {
    name: "Alert",
    docs: "components/display#alert",
    api: &["Alert"],
    overview: &[ex!(overview)],
    examples: &[ex!(tones)],
};
