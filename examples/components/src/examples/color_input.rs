//! Color examples: a hex input with a picker and the app's palette.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// The brand colors the picker offers.
const BRAND: &[&str] = &[
    "#1E88E5",
    "#43A047",
    "#FDD835",
    "#E53935",
    "#8E24AA",
    "#00000080",
];

/// The look of a label.
#[model]
pub struct Look {
    #[rule(required)]
    pub fill: String,
    pub stroke: String,
}

/// Accepts the colors. Nothing is stored.
#[action(public)]
pub async fn save_look(look: Valid<Look>) -> Result<String> {
    Ok(look.fill.clone())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Look {
        fill: "#1E88E5".into(),
        stroke: String::new(),
    })
    .field(Look::fill)
    .control(Look::fill, Control::Color { palette: BRAND })
    .field(Look::stroke)
    .control(Look::stroke, Control::Color { palette: BRAND })
    .submit("Save", save_look)
    .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
    .into()
}

/// What the catalog shows of `Control::Color`.
pub const ENTRY: Entry = Entry {
    name: "Color input",
    docs: "components/controls",
    api: &["Form"],
    overview: &[ex!(overview)],
    examples: &[],
};
