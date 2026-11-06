//! DiffView examples: what changed in a record, with a masked field that stays masked.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// A customer record as the audit trail sees it.
#[model]
pub struct Customer {
    pub name: String,
    pub city: String,
    #[mask(last4)]
    pub iban: String,
    pub credit_limit: Money,
}

fn before() -> Customer {
    Customer {
        name: "Ada Lovelace".into(),
        city: "London".into(),
        iban: "GB33 BUKB 2020 1555 5555 55".into(),
        credit_limit: Money::new(500_000, Currency::Usd),
    }
}

fn after() -> Customer {
    Customer {
        city: "Paris".into(),
        iban: "FR14 2004 1010 0505 0001 3M02 606".into(),
        credit_limit: Money::new(750_000, Currency::Usd),
        ..before()
    }
}

pub fn overview(_ctx: &Ctx) -> Node {
    DiffView::of(&before(), &after())
        .title("Changed on 4 November")
        .into()
}

pub fn inline(_ctx: &Ctx) -> Node {
    DiffView::of(&before(), &after())
        .layout(DiffLayout::Inline)
        .into()
}

/// What the catalog shows of `DiffView`.
pub const ENTRY: Entry = Entry {
    name: "DiffView",
    docs: "components/admin#diffview",
    api: &["DiffView"],
    overview: &[ex!(overview)],
    examples: &[ex!(inline)],
};
