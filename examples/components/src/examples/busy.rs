//! Busy examples: an overlay while a slow action runs.

use crate::data::slow_count;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Busy::new()
        .add(ActionButton::new("Count slowly", slow_count, ()).on_success(|n, ui| {
            if let Ok(n) = n {
                ui.toast(format!("{n} products"));
            }
        }))
        .into()
}

/// What the catalog shows of `Busy`.
pub const ENTRY: Entry = Entry {
    name: "Busy",
    docs: "components/display#tooltip-popover-and-busy-overlay",
    api: &["Busy"],
    overview: &[ex!(overview)],
    examples: &[],
};
