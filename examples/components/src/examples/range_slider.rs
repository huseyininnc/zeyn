//! RangeSlider examples: a price range that drives a count, and a date range.

use crate::data::products;
use crate::{Demo, Entry, exd};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let low = demo.min_price.unwrap_or(0) * 100;
    let high = demo.max_price.unwrap_or(700) * 100;
    let shown = products()
        .iter()
        .filter(|p| (low..=high).contains(&p.price.minor))
        .count();
    Stack::new()
        .add(
            RangeSlider::new("Price", ["min_price", "max_price"], 0, 700)
                .step(10)
                .money(Currency::Try)
                .value(demo.min_price, demo.max_price)
                .drives("prices"),
        )
        .add(Paragraph::new(format!("{shown} of 60 products in this range")).listens("prices"))
        .into()
}

pub fn dates(ctx: &Ctx, _demo: &Demo) -> Node {
    let today = ctx.today();
    RangeSlider::dates(
        "Trip",
        ["from", "to"],
        today,
        today.add_days(60).unwrap_or(today),
    )
    .date_value(today.add_days(7), today.add_days(21))
    .into()
}

/// What the catalog shows of `RangeSlider`.
pub const ENTRY: Entry = Entry {
    name: "RangeSlider",
    docs: "components/commerce#range-slider",
    api: &["RangeSlider"],
    overview: &[exd!(overview)],
    examples: &[exd!(dates)],
};
