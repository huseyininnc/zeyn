//! Control examples: radio buttons, segmented buttons and switches.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// Package size.
#[choice]
pub enum Size {
    Small,
    Medium,
    Large,
}

/// How an order is delivered.
#[choice]
pub enum Delivery {
    Pickup,
    Courier,
    Post,
}

/// Choices of an order.
#[model]
pub struct Choices {
    pub size: Size,
    pub delivery: Delivery,
    pub gift: bool,
}

/// Accepts the choices. Nothing is stored.
#[action(public)]
pub async fn save_choices(choices: Valid<Choices>) -> Result<bool> {
    Ok(choices.gift)
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Choices { size: Size::Medium, ..Choices::default() })
        .field(Choices::size)
        .control(Choices::size, Control::Segmented)
        .field(Choices::delivery)
        .control(Choices::delivery, Control::Radio)
        .field(Choices::gift)
        .control(Choices::gift, Control::Switch)
        .submit("Save", save_choices)
        .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
        .into()
}

/// What the catalog shows of `Control`.
pub const ENTRY: Entry =
    Entry { name: "Controls", docs: "components/controls", api: &["Form"], overview: &[ex!(overview)], examples: &[] };
