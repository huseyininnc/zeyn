//! ComparisonTable examples: plans side by side with a row for each feature.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::CompareCell;

pub fn overview(_ctx: &Ctx) -> Node {
    ComparisonTable::new("Plans")
        .plan("Free")
        .plan("Team")
        .featured()
        .plan("Business")
        .row("Users", ["3", "25", "250"])
        .row("Storage", ["5 GB", "100 GB", "1 TB"])
        .row("Exports", [true, true, true])
        .row("Audit log", [false, true, true])
        .row("Single sign on", [false, false, true])
        .row(
            "Support",
            [
                CompareCell::No,
                CompareCell::from("Email"),
                CompareCell::from("Phone and email"),
            ],
        )
        .into()
}

/// What the catalog shows of `ComparisonTable`.
pub const ENTRY: Entry = Entry {
    name: "ComparisonTable",
    docs: "components/data#comparisontable",
    api: &["ComparisonTable"],
    overview: &[ex!(overview)],
    examples: &[],
};
