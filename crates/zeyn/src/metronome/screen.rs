//! The operations screen of the jobs at `/ops/jobs` (feature `metronome`): jobs, triggers and
//! the run history, built from DataGrids with row actions. Reading needs [`super::JOBS_READ`],
//! running a job now [`super::JOBS_RUN`], pausing and resuming [`super::JOBS_ADMIN`]; the server
//! checks each one for every request. The grids listen to the topic `zeyn.jobs`, which workers
//! notify when a run ends (with the feature `realtime`, at most once a second).

use super::Resume;
use crate::jobs::Jobs;
use crate::prelude::*;

/// Topic the grids of the operations screen listen to.
pub const TOPIC: &str = "zeyn.jobs";

/// A job as the operations screen lists it. Not a table: the grid reads it from the jobs.
#[entity(table = "zeyn_jobs_view")]
pub struct JobRow {
    #[key]
    pub name: String,
    pub state: String,
    pub paused: bool,
    pub next_run: Option<DateTime>,
    pub last_error: String,
    pub runs: i64,
    pub failures: i64,
}

/// A trigger as the operations screen lists it. Not a table: the grid reads it from the jobs.
#[entity(table = "zeyn_triggers_view")]
pub struct TriggerRow {
    #[key]
    pub key: String,
    pub job: String,
    pub id: String,
    pub expr: String,
    pub time_zone: String,
    pub calendar: String,
    pub paused: bool,
    pub next_run: Option<DateTime>,
    pub then: Option<DateTime>,
    pub after: Option<DateTime>,
}

/// One finished try as the operations screen lists it. The payload is never shown.
#[model]
pub struct RunRow {
    #[readonly]
    pub id: String,
    pub job: String,
    pub trigger: String,
    pub scheduled: Option<DateTime>,
    pub started: DateTime,
    pub duration_ms: i64,
    pub attempt: i64,
    pub outcome: String,
    pub error_code: String,
    pub worker: String,
    pub started_by: String,
}

/// A trigger named by its job and id.
#[params]
pub struct TriggerRef {
    pub job: String,
    pub id: String,
}

fn job_rows(jobs: &Jobs) -> Vec<JobRow> {
    jobs.status()
        .into_iter()
        .map(|s| JobRow {
            name: s.name.into(),
            state: format!("{:?}", s.state).to_lowercase(),
            paused: s.paused,
            next_run: s
                .triggers
                .iter()
                .filter(|t| !t.paused)
                .filter_map(|t| t.next_ms.first().copied())
                .min()
                .map(DateTime::from_unix_ms),
            last_error: s.last_error.unwrap_or_default().into(),
            runs: i64::try_from(s.runs).unwrap_or(i64::MAX),
            failures: i64::try_from(s.failures).unwrap_or(i64::MAX),
        })
        .collect()
}

/// The jobs of the app with their state and next run.
#[action(permission = "zeyn.jobs.read", ai = false)]
pub async fn zeyn_jobs_list(q: DataQuery, jobs: Jobs) -> Result<Page<JobRow>> {
    Ok(Page::from_slice(&job_rows(&jobs), &q))
}

/// The triggers of every job with their next three ticks.
#[action(permission = "zeyn.jobs.read", ai = false)]
pub async fn zeyn_jobs_triggers(q: DataQuery, jobs: Jobs) -> Result<Page<TriggerRow>> {
    let rows: Vec<TriggerRow> = jobs
        .names()
        .into_iter()
        .flat_map(|job| {
            jobs.triggers(job)
                .into_iter()
                .map(move |t| {
                    let at = |i: usize| {
                        t.next_ms
                            .get(i)
                            .copied()
                            .map(DateTime::from_unix_ms)
                    };
                    TriggerRow {
                        key: format!("{job}/{}", t.id),
                        job: job.into(),
                        next_run: at(0),
                        then: at(1),
                        after: at(2),
                        id: t.id,
                        expr: t.expr,
                        time_zone: t.time_zone,
                        calendar: t.calendar.unwrap_or_default(),
                        paused: t.paused,
                    }
                })
        })
        .collect();
    Ok(Page::from_slice(&rows, &q))
}

/// The run history, newest first, paged on the server.
#[action(permission = "zeyn.jobs.read", ai = false)]
pub async fn zeyn_jobs_runs(q: DataQuery, jobs: Jobs) -> Result<Page<RunRow>> {
    let span = q.span();
    let limit = usize::try_from(span.limit)
        .unwrap_or(usize::MAX)
        .min(crate::MAX_PAGE_SIZE as usize);
    let mut rows = jobs
        .history(None, span.offset, limit + 1)
        .await?;
    let more = rows.len() > limit;
    rows.truncate(limit);
    let items: Vec<RunRow> = rows
        .into_iter()
        .map(|r| RunRow {
            duration_ms: r.ended_ms.saturating_sub(r.started_ms),
            id: r.id,
            job: r.job,
            trigger: r.trigger_id.unwrap_or_default(),
            scheduled: r
                .scheduled_ms
                .map(DateTime::from_unix_ms),
            started: DateTime::from_unix_ms(r.started_ms),
            attempt: i64::from(r.attempt),
            outcome: r.outcome,
            error_code: r.error_code.unwrap_or_default(),
            worker: r.worker.chars().take(8).collect(),
            started_by: r.started_by.unwrap_or_default(),
        })
        .collect();
    let next = span.offset + items.len() as u64;
    let page = Page::new(items);
    Ok(if more {
        page.next(crate::Cursor(next.to_string()))
    } else {
        page
    })
}

/// Queues a run of the job now, recorded as started by the caller.
#[action(permission = "zeyn.jobs.run", audit, ai = false)]
pub async fn zeyn_jobs_run_now(name: String, jobs: Jobs, ctx: Ctx) -> Result<()> {
    jobs.run_now(&ctx, &name, "")
        .await
        .map(|_| ())
}

/// Pauses the job on every server.
#[action(permission = "zeyn.jobs.admin", audit, ai = false)]
pub async fn zeyn_jobs_pause(name: String, jobs: Jobs) -> Result<()> {
    jobs.pause(&name).await
}

/// Resumes the job at its next tick; ticks of the paused time are dropped.
#[action(permission = "zeyn.jobs.admin", audit, ai = false)]
pub async fn zeyn_jobs_resume(name: String, jobs: Jobs) -> Result<()> {
    jobs.resume(&name, Resume::Skip).await
}

/// Resumes the job and runs its triggers' misfire policies for the paused time.
#[action(permission = "zeyn.jobs.admin", audit, ai = false)]
pub async fn zeyn_jobs_catch_up(name: String, jobs: Jobs) -> Result<()> {
    jobs.resume(&name, Resume::Misfire)
        .await
}

/// Pauses one trigger.
#[action(permission = "zeyn.jobs.admin", audit, ai = false)]
pub async fn zeyn_jobs_pause_trigger(t: TriggerRef, jobs: Jobs) -> Result<()> {
    jobs.pause_trigger(&t.job, &t.id).await
}

/// Resumes one trigger at its next tick.
#[action(permission = "zeyn.jobs.admin", audit, ai = false)]
pub async fn zeyn_jobs_resume_trigger(t: TriggerRef, jobs: Jobs) -> Result<()> {
    jobs.resume_trigger(&t.job, &t.id, Resume::Skip)
        .await
}

/// The operations screen: jobs, triggers and run history.
#[screen(route = "/ops/jobs", permission = "zeyn.jobs.read")]
pub fn zeyn_jobs(_: ()) -> Screen {
    let name = |j: &JobRow| j.name.clone();
    let trigger = |t: &TriggerRow| TriggerRef {
        job: t.job.clone(),
        id: t.id.clone(),
    };
    Screen::new(Text::key("zeyn.jobs.title"))
        .subtitle(Text::key("zeyn.jobs.subtitle"))
        .body(
            Panel::new(Text::key("zeyn.jobs.jobs")).add(
                DataGrid::of(zeyn_jobs_list)
                    .column(JobRow::name)
                    .column(JobRow::state)
                    .column(JobRow::paused)
                    .column(JobRow::next_run)
                    .column(JobRow::last_error)
                    .column(JobRow::runs)
                    .column(JobRow::failures)
                    .row_action(Text::key("zeyn.jobs.run_now"), zeyn_jobs_run_now, name)
                    .row_action(Text::key("zeyn.jobs.pause"), zeyn_jobs_pause, name)
                    .row_action(Text::key("zeyn.jobs.resume"), zeyn_jobs_resume, name)
                    .row_action(Text::key("zeyn.jobs.catch_up"), zeyn_jobs_catch_up, name)
                    .listens(TOPIC),
            ),
        )
        .body(
            Panel::new(Text::key("zeyn.jobs.triggers")).add(
                DataGrid::of(zeyn_jobs_triggers)
                    .column(TriggerRow::job)
                    .column(TriggerRow::id)
                    .column(TriggerRow::expr)
                    .column(TriggerRow::time_zone)
                    .column(TriggerRow::calendar)
                    .column(TriggerRow::paused)
                    .column(TriggerRow::next_run)
                    .column(TriggerRow::then)
                    .column(TriggerRow::after)
                    .row_action(
                        Text::key("zeyn.jobs.pause"),
                        zeyn_jobs_pause_trigger,
                        trigger,
                    )
                    .row_action(
                        Text::key("zeyn.jobs.resume"),
                        zeyn_jobs_resume_trigger,
                        trigger,
                    )
                    .listens(TOPIC),
            ),
        )
        .body(
            Panel::new(Text::key("zeyn.jobs.runs")).add(
                DataGrid::of(zeyn_jobs_runs)
                    .mode(DataMode::Server)
                    .column(RunRow::job)
                    .column(RunRow::trigger)
                    .column(RunRow::scheduled)
                    .column(RunRow::started)
                    .column(RunRow::duration_ms)
                    .column(RunRow::attempt)
                    .column(RunRow::outcome)
                    .column(RunRow::error_code)
                    .column(RunRow::worker)
                    .column(RunRow::started_by)
                    .listens(TOPIC),
            ),
        )
}

/// The operations screen as a feature, with a menu entry: `App::feature(metronome::ops())`.
pub fn ops() -> Feature {
    Feature::new("zeyn_jobs")
        .screen(zeyn_jobs)
        .menu(|m| {
            m.group(Text::key("zeyn.jobs.menu"), Icon::Settings)
                .item(Text::key("zeyn.jobs.title"), zeyn_jobs)
        })
}

/// Texts of the operations screen, kept out of the main catalogs so builds without the feature
/// carry none of them. Merged into the framework's catalog of each language when it is read.
pub(crate) const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../../locales/ops/en.json")),
    ("tr", include_str!("../../locales/ops/tr.json")),
    #[cfg(feature = "lang-zh")]
    ("zh", include_str!("../../locales/ops/zh.json")),
    #[cfg(feature = "lang-ru")]
    ("ru", include_str!("../../locales/ops/ru.json")),
    #[cfg(feature = "lang-es")]
    ("es", include_str!("../../locales/ops/es.json")),
    #[cfg(feature = "lang-ko")]
    ("ko", include_str!("../../locales/ops/ko.json")),
    #[cfg(feature = "lang-ja")]
    ("ja", include_str!("../../locales/ops/ja.json")),
    #[cfg(feature = "lang-hi")]
    ("hi", include_str!("../../locales/ops/hi.json")),
    #[cfg(feature = "lang-ar")]
    ("ar", include_str!("../../locales/ops/ar.json")),
];
