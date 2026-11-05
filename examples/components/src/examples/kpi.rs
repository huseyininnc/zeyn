//! Kpi examples: a number with its change and trend, money, and a row of four.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Kpi::new("Revenue", 184_250.0)
                .delta(12.4)
                .target(200_000.0)
                .trend([140.0, 152.0, 160.0, 184.0]),
        )
        .col(
            Kpi::new("Average wait", 6.5)
                .decimals(1)
                .unit("min")
                .delta(-9.0)
                .lower_is_better(),
        )
        .into()
}

pub fn money(_ctx: &Ctx) -> Node {
    Kpi::money("Revenue this month", Money::tl(18_425_050))
        .delta(4.2)
        .target(200_000.0)
        .into()
}

pub fn row_of_four(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            Kpi::money("Revenue", Money::tl(18_425_050))
                .delta(12.4)
                .trend([140.0, 152.0, 160.0, 184.0]),
        )
        .col(Kpi::new("Orders", 1_284.0).delta(3.1))
        .col(
            Kpi::new("New customers", 312.0)
                .delta(-2.0)
                .trend([330.0, 325.0, 318.0, 312.0]),
        )
        .col(Kpi::money("Average order", Money::tl(14_350)).delta(0.0))
        .into()
}

/// What the catalog shows of `Kpi`.
pub const ENTRY: Entry = Entry {
    name: "Kpi",
    docs: "components/data",
    api: &["Kpi"],
    overview: &[ex!(overview)],
    examples: &[ex!(money), ex!(row_of_four)],
};
