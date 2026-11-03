//! Toast examples: short notices after an action.

use crate::data::pretend_save;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(ActionButton::new("Save", pretend_save, ()).on_success(|_, ui| ui.notify(Toast::success("Saved."))))
        .col(
            ActionButton::new("Warn", pretend_save, ())
                .on_success(|_, ui| ui.notify(Toast::warning("Check the address."))),
        )
        .into()
}

/// What the catalog shows of `Toast`.
pub const ENTRY: Entry =
    Entry { name: "Toast", docs: "language/effects", api: &["Toast"], overview: &[ex!(overview)], examples: &[] };
