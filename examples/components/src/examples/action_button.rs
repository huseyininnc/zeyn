//! ActionButton examples: a button that runs an action.

use crate::data::count_products;
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    ActionButton::new("Count products", count_products, ())
        .on_success(|n, ui| {
            if let Ok(n) = n {
                ui.toast(format!("{n} products"));
            }
        })
        .into()
}

/// What the catalog shows of `ActionButton`.
pub const ENTRY: Entry = Entry {
    name: "ActionButton",
    docs: "components/display#actionbutton",
    api: &["ActionButton"],
    overview: &[ex!(overview)],
    examples: &[],
};
