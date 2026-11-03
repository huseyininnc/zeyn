//! Kpi examples: a number with its change and trend.

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

/// What the catalog shows of `Kpi`.
pub const ENTRY: Entry =
    Entry { name: "Kpi", docs: "components/data", api: &["Kpi"], overview: &[ex!(overview)], examples: &[] };
