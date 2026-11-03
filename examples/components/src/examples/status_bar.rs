//! StatusBar examples: a bar of short status texts.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    StatusBar::new("Status").left("Ready").right("UTF-8").into()
}

/// What the catalog shows of `StatusBar`.
pub const ENTRY: Entry = Entry {
    name: "StatusBar",
    docs: "components/shell",
    api: &["StatusBar"],
    overview: &[ex!(overview)],
    examples: &[],
};
