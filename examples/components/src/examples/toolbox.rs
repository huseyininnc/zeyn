//! Toolbox examples: a group of tool buttons, one selected.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Toolbox::new("Tools")
        .tool("select", "Select", Some(Icon::Search))
        .tool("draw", "Draw", Some(Icon::Edit))
        .tool("text", "Text", None)
        .selected("draw")
        .into()
}

/// What the catalog shows of `Toolbox`.
pub const ENTRY: Entry =
    Entry { name: "Toolbox", docs: "components/shell", api: &["Toolbox"], overview: &[ex!(overview)], examples: &[] };
