//! MenuButton examples: drop down and split buttons.

use crate::data::{count_products, pretend_save};
use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    MenuButton::new("More")
        .action_then("Count products", count_products, (), |n, ui| {
            if let Ok(n) = n {
                ui.toast(format!("{n} products"));
            }
        })
        .separator()
        .action_then("Archive", pretend_save, (), |_, ui| ui.toast("Archived. Nothing was stored."))
        .confirm("Archive the selection?")
        .into()
}

pub fn split(_ctx: &Ctx) -> Node {
    MenuButton::split("Save", pretend_save, ())
        .action_then("Save a copy", pretend_save, (), |_, ui| ui.toast("Saved. Nothing was stored."))
        .into()
}

/// What the catalog shows of `MenuButton`.
pub const ENTRY: Entry = Entry {
    name: "MenuButton",
    docs: "components/display#drop-down-and-split-buttons",
    api: &["MenuButton"],
    overview: &[ex!(overview)],
    examples: &[ex!(split)],
};
