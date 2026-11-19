//! Heatmap examples: a number for each day of the last weeks as shaded squares.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// Sample numbers for the days before `end`: busier on weekdays, with quiet days in between.
fn sample(end: Date, days: i64) -> Vec<(Date, u32)> {
    (0..days)
        .filter_map(|back| {
            let day = end.add_days(-back)?;
            let wave = (back * 7 + back / 3) % 11;
            let value = match (day.weekday(), wave) {
                (6 | 7, _) => wave / 6,
                (_, 0 | 1) => 0,
                (_, wave) => wave,
            };
            Some((day, value as u32))
        })
        .collect()
}

pub fn overview(ctx: &Ctx) -> Node {
    let today = ctx.today();
    Heatmap::ending("Orders a day", today)
        .values(sample(today, 119))
        .into()
}

pub fn a_few_weeks(ctx: &Ctx) -> Node {
    let today = ctx.today();
    Heatmap::ending("Hours logged", today)
        .weeks(8)
        .values(sample(today, 56))
        .tone(Tone::Success)
        .into()
}

/// What the catalog shows of `Heatmap`.
pub const ENTRY: Entry = Entry {
    name: "Heatmap",
    docs: "components/data#heatmap",
    api: &["Heatmap"],
    overview: &[ex!(overview)],
    examples: &[ex!(a_few_weeks)],
};
