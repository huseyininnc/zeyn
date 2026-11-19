//! Scheduled jobs (feature `metronome`): a stock report every minute and on weekday mornings in
//! Istanbul time, and a cleanup chained after it and on its own every 20 seconds. With the feature
//! `metronome-ops` the screen at `/ops/jobs` lists them, their triggers and their runs.

use std::time::Duration;
use zeyn::Schedule;
use zeyn::metronome::{Calendar, Trigger};
use zeyn::prelude::*;

/// Adds the jobs, their triggers and, with `metronome-ops`, the operations screen.
pub fn schedules(app: App) -> App {
    let app = app
        .calendar(
            "holidays",
            Calendar::days(["2026-12-25", "2027-01-01"]).annual([(5, 1)]),
        )
        .queued_job(
            "stock_report",
            Schedule::manual().retry(2, Duration::from_secs(5)),
            stock_report,
        )
        .job("cleanup", Schedule::manual(), cleanup)
        .trigger(
            "stock_report",
            Trigger::cron("0 * * * * *")
                .id("every_minute")
                .time_zone("Europe/Istanbul")
                .payload("all"),
        )
        .trigger(
            "stock_report",
            Trigger::cron("0 30 7 ? * MON-FRI")
                .id("morning")
                .time_zone("Europe/Istanbul")
                .calendar("holidays")
                .priority(5),
        )
        .trigger(
            "cleanup",
            Trigger::cron("*/20 * * * * *")
                .id("often")
                .timeout(Duration::from_secs(5)),
        )
        .chain("stock_report", "cleanup");
    #[cfg(feature = "metronome-ops")]
    let app = app.feature(zeyn::metronome::ops());
    app
}

/// Stands in for a stock report: a real one reads the stock for the part its payload names and
/// the day `ctx.scheduled_at()` gives.
async fn stock_report(_ctx: Ctx, _part: String) -> Result<()> {
    Ok(())
}

/// Removes expired drafts.
async fn cleanup(_ctx: Ctx) -> Result<()> {
    Ok(())
}
