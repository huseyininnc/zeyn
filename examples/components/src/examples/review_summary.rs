//! ReviewSummary examples: the average, the count and a histogram from 5 to 1.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    ReviewSummary::new()
        .count(5, 1_284)
        .count(4, 412)
        .count(3, 96)
        .count(2, 31)
        .count(1, 58)
        .into()
}

pub fn few(_ctx: &Ctx) -> Node {
    ReviewSummary::new()
        .count(4, 2)
        .count(3, 1)
        .into()
}

/// What the catalog shows of `ReviewSummary`.
pub const ENTRY: Entry = Entry {
    name: "ReviewSummary",
    docs: "components/commerce#reviews-and-ratings",
    api: &["ReviewSummary"],
    overview: &[ex!(overview)],
    examples: &[ex!(few)],
};
