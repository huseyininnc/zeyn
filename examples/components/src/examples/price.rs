//! Price examples: a discount with the percent off, a unit price and a large price.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(Price::new(Money::tl(129_900)).was(Money::tl(159_900)))
        .col(Price::new(Money::tl(4_990)))
        .col(Price::new(Money::new(2_499, Currency::Usd)).was(Money::new(2_999, Currency::Usd)))
        .into()
}

pub fn unit_price(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Price::new(Money::tl(12_990))
                .was(Money::tl(14_990))
                .unit(Money::tl(25_980), PriceUnit::Kg),
        )
        .col(Price::new(Money::tl(3_450)).unit(Money::tl(690), PriceUnit::Liter))
        .into()
}

pub fn large(_ctx: &Ctx) -> Node {
    Price::new(Money::tl(1_249_900))
        .was(Money::tl(1_499_900))
        .large()
        .into()
}

/// What the catalog shows of `Price`.
pub const ENTRY: Entry = Entry {
    name: "Price",
    docs: "components/commerce#price",
    api: &["Price"],
    overview: &[ex!(overview)],
    examples: &[ex!(unit_price), ex!(large)],
};
