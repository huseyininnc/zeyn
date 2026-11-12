//! Gantt examples: tasks as bars over a span of days.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(ctx: &Ctx) -> Node {
    let today = ctx.today();
    let day = |offset: i64| today.add_days(offset).unwrap_or(today);
    Gantt::new("Launch plan", day(-20), day(40))
        .task("Research", day(-20), day(-9))
        .done(100)
        .tone(Tone::Success)
        .task("Design", day(-12), day(4))
        .done(70)
        .task("Build", day(-2), day(26))
        .done(15)
        .task("Test", day(18), day(34))
        .tone(Tone::Info)
        .task("Launch", day(36), day(40))
        .tone(Tone::Warning)
        .today(today)
        .into()
}

/// What the catalog shows of `Gantt`.
pub const ENTRY: Entry = Entry {
    name: "Gantt",
    docs: "components/data#gantt",
    api: &["Gantt"],
    overview: &[ex!(overview)],
    examples: &[],
};
