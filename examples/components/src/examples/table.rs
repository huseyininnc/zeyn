//! Table examples: a small table of text the screen already holds.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Table::new("Opening hours")
        .head(["Day", "Opens", "Closes"])
        .row(["Monday to Friday", "09:00", "18:00"])
        .row(["Saturday", "10:00", "14:00"])
        .row(["Sunday", "Closed", ""])
        .row_headers()
        .into()
}

pub fn numbers_and_stripes(_ctx: &Ctx) -> Node {
    Table::new("Plans")
        .head(["Plan", "Users", "Storage (GB)", "Price a month"])
        .row(["Free", "3", "5", "₺0"])
        .row(["Team", "25", "100", "₺490"])
        .row(["Business", "250", "1,000", "₺2,900"])
        .row(["Enterprise", "Unlimited", "10,000", "₺9,500"])
        .row_headers()
        .numeric(1)
        .numeric(2)
        .numeric(3)
        .striped()
        .into()
}

/// What the catalog shows of `Table`.
pub const ENTRY: Entry = Entry {
    name: "Table",
    docs: "components/data#table",
    api: &["Table"],
    overview: &[ex!(overview)],
    examples: &[ex!(numbers_and_stripes)],
};
