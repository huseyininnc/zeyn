//! Length examples: a number with a unit switch, stored in pixels.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// The size of a report band.
#[model]
pub struct Band {
    #[rule(required, min = 0, max = 2000, scale = 4)]
    pub height: Option<Decimal>,
    #[rule(min = 0, max = 200, scale = 4)]
    pub padding: Decimal,
}

/// Accepts the lengths. Nothing is stored.
#[action(public)]
pub async fn save_band(band: Valid<Band>) -> Result<String> {
    Ok(band
        .height
        .unwrap_or_default()
        .to_string())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Band {
        height: Decimal::parse("113.3858"),
        padding: Decimal::parse("7.5591").unwrap_or_default(),
    })
    .field(Band::height)
    .control(
        Band::height,
        Control::Length {
            units: &[
                LengthUnit::Mm,
                LengthUnit::Cm,
                LengthUnit::In,
                LengthUnit::Pt,
            ],
        },
    )
    .field(Band::padding)
    .control(Band::padding, Control::Length { units: &[] })
    .submit("Save", save_band)
    .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
    .into()
}

/// What the catalog shows of `Control::Length`.
pub const ENTRY: Entry = Entry {
    name: "Length input",
    docs: "components/controls",
    api: &["LengthUnit"],
    overview: &[ex!(overview)],
    examples: &[],
};
