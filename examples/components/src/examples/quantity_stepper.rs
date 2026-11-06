//! QuantityStepper examples: a cart quantity whose stock the server checks.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{QuantityChange, QuantityState};

/// Units in stock of the sample items. Kept quantities go to the capped [`crate::data::CART`].
fn stock(target: &str) -> Option<i64> {
    match target {
        "mug" => Some(5),
        "tea" => Some(40),
        _ => None,
    }
}

/// Keeps a quantity up to the stock of the item.
#[action(public)]
pub async fn set_cart_quantity(change: QuantityChange) -> Result<QuantityState> {
    let stock = stock(&change.target).ok_or(Error::NotFound)?;
    let kept = change.quantity.min(stock);
    crate::data::CART.push((change.target, kept));
    Ok(QuantityState::new(kept).max(stock))
}

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            QuantityStepper::of(set_cart_quantity, "mug")
                .label("Mugs, 5 in stock")
                .range(1, 10)
                .value(2),
        )
        .col(
            QuantityStepper::of(set_cart_quantity, "tea")
                .label("Tea")
                .range(0, 99)
                .value(1),
        )
        .into()
}

/// What the catalog shows of `QuantityStepper`.
pub const ENTRY: Entry = Entry {
    name: "QuantityStepper",
    docs: "components/commerce#quantity-stepper",
    api: &["QuantityStepper", "QuantityState"],
    overview: &[ex!(overview)],
    examples: &[],
};
