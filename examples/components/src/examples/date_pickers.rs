//! Date and time picker examples.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// A visit booked with the pickers.
#[model]
pub struct Visit {
    #[rule(required)]
    pub day: Option<Date>,
    pub arrives: Time,
    pub stay_from: Option<Date>,
    pub stay_to: Option<Date>,
}

/// Accepts a valid visit. Nothing is stored.
#[action(public)]
pub async fn book_visit(visit: Valid<Visit>) -> Result<Visit> {
    Ok(visit.into_inner())
}

pub fn overview(ctx: &Ctx) -> Node {
    Form::of(Visit { day: Some(ctx.today()), ..Visit::default() })
        .row(|r| {
            r.field(Visit::day)
                .field(Visit::arrives)
        })
        .range(Visit::stay_from, Visit::stay_to)
        .date_options(
            Visit::day,
            DateOptions::new()
                .no_weekends()
                .week_numbers(),
        )
        .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
        .submit("Book", book_visit)
        .on_success(|_, ui| ui.toast("Booked. Nothing was stored."))
        .into()
}

/// What the catalog shows of `DateOptions`.
pub const ENTRY: Entry = Entry {
    name: "Date pickers",
    docs: "components/controls",
    api: &["DateOptions"],
    overview: &[ex!(overview)],
    examples: &[],
};
