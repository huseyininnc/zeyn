//! Metronome: wall clock schedules on top of the job system (feature `metronome`).
//!
//! A [`Trigger`] decides *when* a run of a registered job is queued; the existing claim, lease,
//! retry and concurrency code of [`crate::Jobs`] still runs it. A job can have several triggers,
//! each with a cron expression (seconds first, Quartz dialect, see [`Cron`]), a time zone, a
//! payload, a priority, a timeout, a [`Calendar`] of excluded days and times, and a [`Misfire`]
//! policy for ticks found late. Each trigger keeps only its next tick, one `i64`; the store moves
//! it with a compare and set in the same transaction that queues the runs, so servers that share
//! a store queue every tick exactly once.
//!
//! ```ignore
//! App::new("Billing")
//!     .job_store(SqlJobStore::new(db.clone()))
//!     .calendar("tr-holidays", Calendar::days(["2026-01-01", "2026-04-23"]).weekly_off([Weekday::Saturday, Weekday::Sunday]))
//!     .queued_job("close_day", Schedule::manual().retry(3, Duration::from_secs(30)), close_day)
//!     .trigger("close_day", Trigger::cron("0 30 23 * * *").id("nightly").time_zone("Europe/Istanbul").calendar("tr-holidays"))
//!     .trigger("close_day", Trigger::cron("0 0 12 L * ?").id("month_end").payload("month").priority(10).misfire(Misfire::RunAll { max: 3 }))
//!     .chain("close_day", "send_reports")
//! ```
//!
//! See `docs/metronome.md`.

mod cron;
#[cfg(feature = "metronome-ops")]
pub(crate) mod screen;

pub use cron::{Cron, MAX_YEAR, MIN_YEAR};
#[cfg(feature = "metronome-ops")]
pub use screen::{
    JobRow, RunRow as RunView, TOPIC, TriggerRef, TriggerRow as TriggerView, ops, zeyn_jobs,
    zeyn_jobs_catch_up, zeyn_jobs_list, zeyn_jobs_pause, zeyn_jobs_pause_trigger, zeyn_jobs_resume,
    zeyn_jobs_resume_trigger, zeyn_jobs_run_now, zeyn_jobs_runs, zeyn_jobs_triggers,
};

use crate::action::{Error, Result};
use crate::jobs::{Jobs, NewJob};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Most triggers one app may register.
pub const MAX_TRIGGERS: usize = 1_000;

/// Most runs one trigger queues for missed ticks in one pass ([`Misfire::RunAll`]).
pub const MAX_CATCH_UP: u32 = 100;

/// Most ticks looked at for one trigger in one pass. A trigger further behind (a schedule of
/// every second after a long downtime) jumps to its next future tick; skipped counts then stop
/// at this number.
pub const MAX_MISFIRE_SCAN: u32 = 10_000;

/// Longest chain of jobs started one after another by [`crate::App::chain`].
pub const MAX_CHAIN: usize = 8;

/// Most excluded days in one [`Calendar`].
pub const MAX_CALENDAR_DAYS: usize = 3_660;

/// How late a tick may be found before it counts as misfired, unless the app sets another.
pub const MISFIRE_THRESHOLD: Duration = Duration::from_secs(60);

/// A due run that has waited this long is claimed before runs of a higher priority.
pub const PRIORITY_AGING: Duration = Duration::from_secs(300);

/// How often a scheduler reads triggers and pauses from the store, to see changes another
/// server made.
pub const TRIGGER_REFRESH: Duration = Duration::from_secs(30);

/// How often a scheduler deletes run history past the retention, at most
/// [`PRUNE_BATCH`] rows each time.
pub const PRUNE_EVERY: Duration = Duration::from_secs(600);

/// Most history rows one pruning pass deletes, so it never holds a long lock.
pub const PRUNE_BATCH: usize = 1_000;

/// History rows a `MemoryJobStore` keeps, the newest.
pub const MEMORY_HISTORY_CAPACITY: usize = 1_000;

/// Permission to see the operations screen (`/ops/jobs`).
pub const JOBS_READ: &str = "zeyn.jobs.read";

/// Permission to run a job now from the operations screen.
pub const JOBS_RUN: &str = "zeyn.jobs.run";

/// Permission to pause and resume jobs and triggers from the operations screen.
pub const JOBS_ADMIN: &str = "zeyn.jobs.admin";

/// Where the scheduler reads the time. [`SystemClock`] in production; [`FakeClock`] in tests,
/// which moves only when the test says so.
pub trait Clock: Send + Sync + 'static {
    /// Now, as Unix milliseconds.
    fn now_ms(&self) -> i64;
}

/// The system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        crate::time::DateTime::now().unix_ms()
    }
}

/// A clock for tests that moves only when told to. Clones share the time.
///
/// ```
/// use zeyn::metronome::{Clock, FakeClock};
/// let clock = FakeClock::at(1_000);
/// clock.advance(std::time::Duration::from_secs(2));
/// assert_eq!(clock.now_ms(), 3_000);
/// ```
#[derive(Clone, Default)]
pub struct FakeClock(Arc<AtomicI64>);

impl FakeClock {
    /// A clock standing at `ms` (Unix milliseconds).
    pub fn at(ms: i64) -> Self {
        FakeClock(Arc::new(AtomicI64::new(ms)))
    }

    /// Sets the time.
    pub fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::SeqCst);
    }

    /// Moves the time forward.
    pub fn advance(&self, by: Duration) {
        let ms = i64::try_from(by.as_millis()).unwrap_or(i64::MAX);
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// What a trigger does with ticks found later than the misfire threshold (servers down,
/// paused or saturated).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Misfire {
    /// One run now for all missed ticks, with the latest missed tick as its time.
    #[default]
    RunOnce,
    /// One run per missed tick, oldest first, at most `max` (capped at [`MAX_CATCH_UP`]); the
    /// rest are recorded as one `skipped` history row.
    RunAll { max: u32 },
    /// No run for missed ticks; one `skipped` history row records how many.
    Skip,
}

impl Misfire {
    fn code(self) -> &'static str {
        match self {
            Misfire::RunOnce => "run_once",
            Misfire::RunAll { .. } => "run_all",
            Misfire::Skip => "skip",
        }
    }
}

/// How a paused job or trigger picks up again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resume {
    /// Moves to the next tick after now; ticks of the paused time are dropped.
    #[default]
    Skip,
    /// Applies the trigger's [`Misfire`] policy to the ticks of the paused time.
    Misfire,
}

/// A day of the week, for [`Calendar::weekly_off`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weekday {
    Monday = 1,
    Tuesday = 2,
    Wednesday = 3,
    Thursday = 4,
    Friday = 5,
    Saturday = 6,
    Sunday = 7,
}

/// Days and times when a trigger does not run: holidays, weekends, maintenance windows. A tick
/// that falls in an excluded day or range is not run and does not count as a misfire. Dates and
/// times are read in the trigger's time zone.
///
/// ```
/// use zeyn::metronome::{Calendar, Weekday};
/// let closed = Calendar::days(["2026-01-01", "2026-04-23"])
///     .weekly_off([Weekday::Saturday, Weekday::Sunday])
///     .annual([(5, 1)])
///     .ranges([("23:30", "00:30")]);
/// assert!(closed.excludes(zeyn::DateTime::parse("2026-01-01T12:00:00Z").unwrap(), zeyn::TimeZone::UTC));
/// assert!(!closed.excludes(zeyn::DateTime::parse("2026-01-02T12:00:00Z").unwrap(), zeyn::TimeZone::UTC));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Calendar {
    days: Vec<i64>,
    weekly: u8,
    annual: Vec<(u8, u8)>,
    ranges: Vec<(u32, u32)>,
    error: Option<String>,
}

impl Calendar {
    /// Excludes whole dates written as `2026-01-01`.
    pub fn days<I, S>(days: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut c = Calendar::default();
        for d in days {
            match crate::time::Date::parse(d.as_ref()) {
                Some(date) => c.days.push(date.days_since_epoch()),
                None => {
                    c.error = Some(format!(
                        "`{}` is not a date; write days as `2026-01-01`",
                        d.as_ref()
                    ));
                }
            }
        }
        c.days.sort_unstable();
        c.days.dedup();
        if c.days.len() > MAX_CALENDAR_DAYS {
            c.error = Some(format!(
                "a calendar holds at most {MAX_CALENDAR_DAYS} days; split it or use `annual` for dates that repeat"
            ));
        }
        c
    }

    /// Excludes these days of every week.
    pub fn weekly_off(mut self, days: impl IntoIterator<Item = Weekday>) -> Self {
        for d in days {
            self.weekly |= 1 << (d as u8);
        }
        self
    }

    /// Excludes dates that repeat every year, as `(month, day)`.
    pub fn annual(mut self, dates: impl IntoIterator<Item = (u8, u8)>) -> Self {
        for (m, d) in dates {
            if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
                self.error = Some(format!(
                    "`({m}, {d})` is not a month and day; write `(5, 1)` for May 1"
                ));
            }
            self.annual.push((m, d));
        }
        self
    }

    /// Excludes daily time ranges written as `("23:30", "00:30")`: from the start, up to but not
    /// including the end. A range whose end comes before its start crosses midnight.
    pub fn ranges<'a>(mut self, ranges: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        for (from, to) in ranges {
            match (crate::time::Time::parse(from), crate::time::Time::parse(to)) {
                (Some(a), Some(b)) => self
                    .ranges
                    .push((secs_of(a), secs_of(b))),
                _ => {
                    self.error = Some(format!(
                        "`{from}` to `{to}` is not a time range; write times as `23:30`"
                    ));
                }
            }
        }
        self
    }

    /// Excludes what either calendar excludes.
    pub fn and(mut self, other: Calendar) -> Self {
        self.days.extend(other.days);
        self.days.sort_unstable();
        self.days.dedup();
        self.weekly |= other.weekly;
        self.annual.extend(other.annual);
        self.ranges.extend(other.ranges);
        self.error = self.error.or(other.error);
        self
    }

    /// Whether the calendar excludes this moment in `zone`.
    pub fn excludes(&self, at: crate::time::DateTime, zone: crate::TimeZone) -> bool {
        let local = at.local(zone);
        let date = local.date;
        let second = secs_of(local.time);
        self.days
            .binary_search(&date.days_since_epoch())
            .is_ok()
            || self.weekly >> date.weekday() & 1 == 1
            || self
                .annual
                .contains(&(date.month(), date.day()))
            || self.ranges.iter().any(|(a, b)| {
                if a <= b {
                    (*a..*b).contains(&second)
                } else {
                    second >= *a || second < *b
                }
            })
    }
}

fn secs_of(t: crate::time::Time) -> u32 {
    u32::from(t.hour()) * 3600 + u32::from(t.minute()) * 60 + u32::from(t.second())
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Cron(Cron),
    Every(i64),
    Once(i64),
}

impl Kind {
    fn next(&self, after: i64, zone: &jiff::tz::TimeZone) -> i64 {
        match self {
            Kind::Cron(c) => c
                .next_in(after, zone)
                .unwrap_or(i64::MAX),
            Kind::Every(d) => after.saturating_add(*d),
            Kind::Once(at) if *at > after => *at,
            Kind::Once(_) => i64::MAX,
        }
    }
}

/// When a job is queued: a cron expression, an interval or one moment, with a time zone,
/// payload, priority, timeout, calendar and misfire policy. Register it with
/// [`crate::App::trigger`]; mistakes are reported when the app starts, with the trigger named.
///
/// ```
/// use std::time::Duration;
/// use zeyn::metronome::{Misfire, Trigger};
/// let t = Trigger::cron("0 30 23 * * *")
///     .id("nightly")
///     .time_zone("Europe/Istanbul")
///     .misfire(Misfire::RunAll { max: 3 })
///     .timeout(Duration::from_secs(900));
/// assert!(t.problem().is_none());
/// assert!(Trigger::cron("30 23 * * *").problem().unwrap().contains("seconds"));
/// ```
///
/// A schedule is a parsed `Trigger`, never a bare string, so every expression is checked at
/// startup:
///
/// ```compile_fail
/// zeyn::App::new("T").trigger("report", "0 0 * * * *");
/// ```
///
/// A catch up names its cap, and a timeout is a `Duration`, not a number of some unit:
///
/// ```compile_fail
/// zeyn::metronome::Trigger::cron("0 0 * * * *").misfire(zeyn::metronome::Misfire::RunAll);
/// ```
///
/// ```compile_fail
/// zeyn::metronome::Trigger::cron("0 0 * * * *").timeout(900);
/// ```
#[derive(Clone, Debug)]
pub struct Trigger {
    pub(crate) kind: Option<Kind>,
    pub(crate) source: String,
    pub(crate) id: Option<String>,
    pub(crate) zone: String,
    pub(crate) calendar: Option<String>,
    pub(crate) payload: String,
    pub(crate) priority: i16,
    pub(crate) misfire: Misfire,
    pub(crate) timeout: Option<Duration>,
    pub(crate) error: Option<String>,
}

impl Trigger {
    fn of(kind: std::result::Result<Kind, String>, source: String) -> Self {
        let (kind, error) = match kind {
            Ok(k) => (Some(k), None),
            Err(e) => (None, Some(e)),
        };
        Trigger {
            kind,
            source,
            id: None,
            zone: "UTC".into(),
            calendar: None,
            payload: String::new(),
            priority: 0,
            misfire: Misfire::default(),
            timeout: None,
            error,
        }
    }

    /// A cron expression with seconds (see [`Cron`]).
    pub fn cron(expr: &str) -> Self {
        Trigger::of(Cron::parse(expr).map(Kind::Cron), expr.trim().into())
    }

    /// Every `interval`, the first time one interval after the trigger is first stored.
    pub fn every(interval: Duration) -> Self {
        let ms = i64::try_from(interval.as_millis()).unwrap_or(i64::MAX);
        let kind = if ms >= 1_000 {
            Ok(Kind::Every(ms))
        } else {
            Err("an interval trigger runs at most once a second".into())
        };
        Trigger::of(kind, format!("every {}s", ms / 1000))
    }

    /// Once, at this moment.
    pub fn once(at: crate::time::DateTime) -> Self {
        Trigger::of(
            Ok(Kind::Once(at.unix_ms())),
            format!("once {}", at.unix_ms()),
        )
    }

    /// The trigger's name: 1 to 64 of `a-z`, `0-9`, `_` and `-`. Without one it is `cron:<n>`
    /// in registration order, stable as long as the code is.
    pub fn id(mut self, id: &str) -> Self {
        if !valid_id(id) {
            self.error = Some(format!(
                "`{id}` is not a trigger id; use 1 to 64 of a-z, 0-9, `_` and `-`"
            ));
        }
        self.id = Some(id.into());
        self
    }

    /// IANA time zone the expression is read in (`Europe/Istanbul`); UTC unless set.
    pub fn time_zone(mut self, zone: &str) -> Self {
        if crate::TimeZone::named(zone).is_none() {
            self.error = Some(format!(
                "`{zone}` is not a time zone; use an IANA name such as `Europe/Istanbul`"
            ));
        }
        self.zone = zone.into();
        self
    }

    /// Days and times when the trigger does not run, by the name given to `App::calendar`.
    pub fn calendar(mut self, name: &str) -> Self {
        self.calendar = Some(name.into());
        self
    }

    /// Text passed to a `queued_job`, at most [`crate::MAX_JOB_PAYLOAD`] bytes. Keep it an id;
    /// the job reads the record.
    pub fn payload(mut self, payload: impl Into<String>) -> Self {
        let payload = payload.into();
        if payload.len() > crate::MAX_JOB_PAYLOAD {
            self.error = Some(format!(
                "a trigger payload is at most {} bytes",
                crate::MAX_JOB_PAYLOAD
            ));
        }
        self.payload = payload;
        self
    }

    /// Order between due runs: higher first, 0 unless set. A run that waited longer than
    /// [`PRIORITY_AGING`] goes first whatever its priority, so none starves.
    pub fn priority(mut self, priority: i16) -> Self {
        self.priority = priority;
        self
    }

    /// What happens to ticks found late. [`Misfire::RunOnce`] unless set.
    pub fn misfire(mut self, misfire: Misfire) -> Self {
        self.misfire = misfire;
        self
    }

    /// Longest time one try may take; past it the try fails with the error code `timeout` and is
    /// retried as the job's schedule says. No limit unless set.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// The configuration mistake of this trigger, if any, written for whoever fixes it.
    pub fn problem(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// How long run history is kept: by age and by count, whichever deletes more.
///
/// ```
/// let keep = zeyn::metronome::Retention::days(30).max_rows(100_000);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retention {
    pub(crate) days: u32,
    pub(crate) max_rows: u64,
}

impl Default for Retention {
    fn default() -> Self {
        Retention {
            days: 30,
            max_rows: 100_000,
        }
    }
}

impl Retention {
    /// Keeps rows of the last `days` days, at most 100,000 unless [`Retention::max_rows`] says.
    pub fn days(days: u32) -> Self {
        Retention {
            days: days.max(1),
            ..Retention::default()
        }
    }

    /// Keeps at most this many rows, the newest.
    pub fn max_rows(mut self, rows: u64) -> Self {
        self.max_rows = rows.max(1);
        self
    }
}

/// A trigger as the store keeps it: its definition and its next tick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TriggerRow {
    pub job: String,
    pub id: String,
    /// The expression in its source text (`0 30 23 * * *`, `every 60s`).
    pub expr: String,
    pub time_zone: String,
    pub calendar: Option<String>,
    /// `run_once`, `run_all` or `skip`.
    pub misfire: String,
    pub misfire_max: Option<u32>,
    pub priority: i16,
    pub paused: bool,
    /// The next tick, Unix milliseconds; `i64::MAX` when there is none.
    pub next_ms: i64,
}

/// One finished try of a run, or one record of ticks skipped by a misfire policy. Payloads and
/// error texts are never kept; the error is a code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RunRow {
    pub id: String,
    pub run_id: String,
    pub job: String,
    /// `manual`, `chain`, a trigger id, or `None` for runs queued with `Jobs::enqueue`.
    pub trigger_id: Option<String>,
    pub scheduled_ms: Option<i64>,
    pub started_ms: i64,
    pub ended_ms: i64,
    /// The try, from 1; for a `skipped` row, the number of ticks skipped (counted up to
    /// [`MAX_MISFIRE_SCAN`]).
    pub attempt: u32,
    /// `succeeded`, `failed`, `timeout` or `skipped`.
    pub outcome: String,
    pub error_code: Option<String>,
    pub worker: String,
    pub started_by: Option<String>,
}

/// The status of one trigger, built on request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TriggerStatus {
    pub id: String,
    pub expr: String,
    pub time_zone: String,
    pub calendar: Option<String>,
    pub paused: bool,
    /// The next ticks, Unix milliseconds, at most three.
    pub next_ms: Vec<i64>,
}

struct Slot {
    job: &'static str,
    id: Arc<str>,
    trigger: Arc<Trigger>,
    kind: Kind,
    zone: jiff::tz::TimeZone,
    tz: crate::TimeZone,
    calendar: Option<Arc<Calendar>>,
    next: i64,
    paused: bool,
}

#[derive(Default)]
struct State {
    pending: Vec<(String, Trigger)>,
    problems: Vec<String>,
    calendars: Vec<(String, Arc<Calendar>)>,
    chains: Vec<(String, String)>,
    slots: Vec<Slot>,
    paused: HashSet<String>,
    loaded: bool,
    refreshed: i64,
    pruned: i64,
    retention: Retention,
    threshold: Option<Duration>,
}

/// The scheduler part of [`Jobs`]: triggers, calendars, chains, pauses and the clock.
pub(crate) struct Scheduler {
    state: Mutex<State>,
    clock: Mutex<Arc<dyn Clock>>,
    #[cfg_attr(not(feature = "realtime"), allow(dead_code))]
    notified: AtomicI64,
}

impl Default for Scheduler {
    fn default() -> Self {
        Scheduler {
            state: Mutex::default(),
            clock: Mutex::new(Arc::new(SystemClock)),
            notified: AtomicI64::new(0),
        }
    }
}

impl Scheduler {
    pub(crate) fn now(&self) -> i64 {
        self.clock
            .lock()
            .map(|c| c.now_ms())
            .unwrap_or_else(|_| SystemClock.now_ms())
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| Error::Internal("scheduler lock".into()))
    }

    pub(crate) fn chained(&self, job: &str) -> Vec<String> {
        self.state()
            .map(|s| {
                s.chains
                    .iter()
                    .filter(|(a, _)| a == job)
                    .map(|(_, b)| b.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn is_paused(&self, job: &str) -> bool {
        self.state()
            .is_ok_and(|s| s.paused.contains(job))
    }

    pub(crate) fn has_triggers(&self) -> bool {
        self.state()
            .is_ok_and(|s| !s.pending.is_empty())
    }

    /// Whether a live `zeyn.jobs` notification may go out now: at most one per second.
    #[cfg_attr(not(feature = "realtime"), allow(dead_code))]
    pub(crate) fn may_notify(&self, now: i64) -> bool {
        let last = self.notified.load(Ordering::SeqCst);
        now - last >= 1_000
            && self
                .notified
                .compare_exchange(last, now, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
    }
}

fn kind_of(t: &Trigger) -> Option<Kind> {
    t.kind.clone()
}

/// Finds a cycle or a chain longer than [`MAX_CHAIN`], from `start`.
fn chain_problem(
    chains: &[(String, String)],
    start: &str,
    path: &mut Vec<String>,
) -> Option<String> {
    if path.iter().any(|p| p == start) {
        path.push(start.into());
        return Some(format!(
            "the job chain {} is a cycle; a chain runs each job once, so remove one link",
            path.join(" -> ")
        ));
    }
    path.push(start.into());
    if path.len() > MAX_CHAIN {
        return Some(format!(
            "the job chain {} is longer than {MAX_CHAIN} jobs; start the rest from a job of its own",
            path.join(" -> ")
        ));
    }
    for (_, b) in chains
        .iter()
        .filter(|(a, _)| a == start)
    {
        if let Some(p) = chain_problem(chains, b, path) {
            return Some(p);
        }
    }
    path.pop();
    None
}

fn problem(summary: String, fix: &str) -> String {
    crate::Problem::new("Z0508", summary)
        .why("the scheduler would start without it, so the job would not run when the code says")
        .fix(fix)
        .docs("guides/enterprise#metronome")
        .to_string()
}

impl Jobs {
    /// Uses `clock` for every time the jobs read: due runs, ticks, leases, history. For tests
    /// with a [`FakeClock`]; set it before the jobs start.
    pub fn set_clock(&self, clock: impl Clock) {
        if let Ok(mut c) = self.scheduler().clock.lock() {
            *c = Arc::new(clock);
        }
    }

    /// Registers a trigger of `job`. Mistakes are kept and reported by [`Jobs::problems`].
    pub fn add_trigger(&self, job: &str, trigger: Trigger) {
        let Ok(mut s) = self.scheduler().state() else {
            return;
        };
        let mut trigger = trigger;
        let n = s
            .pending
            .iter()
            .filter(|(j, _)| j == job)
            .count();
        let id = trigger
            .id
            .get_or_insert_with(|| format!("cron:{n}"))
            .clone();
        if let Some(e) = &trigger.error {
            s.problems.push(problem(
                format!("trigger `{id}` of job `{job}`: {e}"),
                "change the trigger as the message says",
            ));
        }
        if s.pending
            .iter()
            .any(|(j, t)| j == job && t.id.as_deref() == Some(id.as_str()))
        {
            s.problems.push(problem(
                format!("job `{job}` has two triggers with the id `{id}`"),
                "give each trigger of a job its own `.id(...)`",
            ));
        }
        if s.pending.len() >= MAX_TRIGGERS {
            s.problems.push(problem(
                format!("more than {MAX_TRIGGERS} triggers"),
                "merge triggers with the same schedule, or loop over records inside one job",
            ));
            return;
        }
        s.pending.push((job.into(), trigger));
    }

    /// Registers a calendar that triggers name with [`Trigger::calendar`].
    pub fn add_calendar(&self, name: &str, calendar: Calendar) {
        let Ok(mut s) = self.scheduler().state() else {
            return;
        };
        if let Some(e) = &calendar.error {
            s.problems.push(problem(
                format!("calendar `{name}`: {e}"),
                "change the calendar as the message says",
            ));
        }
        s.calendars.retain(|(n, _)| n != name);
        s.calendars
            .push((name.into(), Arc::new(calendar)));
    }

    /// After a run of `first` succeeds, queues a run of `then` with the same payload.
    pub fn add_chain(&self, first: &str, then: &str) {
        if let Ok(mut s) = self.scheduler().state() {
            s.chains
                .push((first.into(), then.into()));
        }
    }

    /// How long run history is kept. 30 days and 100,000 rows unless set.
    pub fn set_retention(&self, retention: Retention) {
        if let Ok(mut s) = self.scheduler().state() {
            s.retention = retention;
        }
    }

    /// How late a tick may be found before its trigger's [`Misfire`] policy applies; one
    /// minute unless set.
    pub fn set_misfire_threshold(&self, threshold: Duration) {
        if let Ok(mut s) = self.scheduler().state() {
            s.threshold = Some(threshold);
        }
    }

    /// Mistakes in triggers, calendars and chains: unknown jobs and calendars, bad expressions,
    /// duplicate ids, cycles. `App` reports them at startup.
    pub fn problems(&self) -> Vec<String> {
        let names = self.names();
        let Ok(s) = self.scheduler().state() else {
            return Vec::new();
        };
        let mut out = s.problems.clone();
        for (job, t) in &s.pending {
            let id = t.id.as_deref().unwrap_or_default();
            if !names.iter().any(|n| n == job) {
                out.push(problem(
                    format!("trigger `{id}` names the job `{job}`, which is not registered"),
                    "register the job with `App::job` or `App::queued_job`, or fix the name",
                ));
            }
            if let Some(c) = &t.calendar
                && !s.calendars.iter().any(|(n, _)| n == c)
            {
                out.push(problem(
                    format!("trigger `{id}` of job `{job}` names the calendar `{c}`, which is not registered"),
                    "add it with `App::calendar`, or fix the name",
                ));
            }
            if let (Some(kind), Ok(zone)) = (&t.kind, zone_of(&t.zone))
                && kind.next(0, &zone.jiff()) == i64::MAX
                && !matches!(kind, Kind::Once(_))
            {
                out.push(problem(
                    format!("trigger `{id}` of job `{job}` never runs (`{}`)", t.source),
                    "check the day, month and year fields",
                ));
            }
        }
        for (a, b) in &s.chains {
            for j in [a, b] {
                if !names.iter().any(|n| n == j) {
                    out.push(problem(
                        format!(
                            "the chain `{a}` -> `{b}` names the job `{j}`, which is not registered"
                        ),
                        "register the job, or fix the name",
                    ));
                }
            }
        }
        let starts: HashSet<&String> = s
            .chains
            .iter()
            .map(|(a, _)| a)
            .collect();
        for a in starts {
            if let Some(p) = chain_problem(&s.chains, a, &mut Vec::new()) {
                out.push(problem(p, "change `App::chain` as the message says"));
                break;
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Compiles the triggers of registered jobs into scheduler slots, once.
    fn compile(&self) {
        let names = self.names();
        let Ok(mut s) = self.scheduler().state() else {
            return;
        };
        if !s.slots.is_empty() {
            return;
        }
        let mut slots = Vec::new();
        for (job, t) in &s.pending {
            let (Some(job), Some(kind), Ok(tz)) = (
                names
                    .iter()
                    .find(|n| **n == job.as_str()),
                kind_of(t),
                zone_of(&t.zone),
            ) else {
                continue;
            };
            if t.error.is_some() {
                continue;
            }
            let calendar = t.calendar.as_ref().and_then(|c| {
                s.calendars
                    .iter()
                    .find(|(n, _)| n == c)
                    .map(|(_, c)| c.clone())
            });
            slots.push(Slot {
                job,
                id: Arc::from(t.id.as_deref().unwrap_or_default()),
                trigger: Arc::new(t.clone()),
                kind,
                zone: tz.jiff(),
                tz,
                calendar,
                next: i64::MAX,
                paused: false,
            });
        }
        s.slots = slots;
    }

    /// Runs one scheduler pass now: queues the runs of every due trigger, applying calendars
    /// and misfire policies, and returns how many it queued. The scheduler loop does this every
    /// [`crate::JOB_POLL`]; tests with a [`FakeClock`] call it after moving the clock.
    pub async fn run_due(&self) -> Result<usize> {
        self.compile();
        let sch = self.scheduler();
        let now = sch.now();
        let store = self.store_handle();
        let (loaded, refreshed) = {
            let s = sch.state()?;
            (s.loaded, s.refreshed)
        };
        if !loaded {
            let rows: Vec<TriggerRow> = {
                let s = sch.state()?;
                s.slots
                    .iter()
                    .map(|slot| row_of(slot, slot.kind.next(now, &slot.zone)))
                    .collect()
            };
            let mut stored = Vec::with_capacity(rows.len());
            for row in rows {
                stored.push(store.add_trigger(row).await?);
            }
            let paused = store.paused_jobs().await?;
            let mut s = sch.state()?;
            apply_rows(&mut s, &stored, paused, now);
            s.loaded = true;
        } else if now - refreshed >= ms(TRIGGER_REFRESH) {
            let rows = store.triggers().await?;
            let paused = store.paused_jobs().await?;
            let mut s = sch.state()?;
            apply_rows(&mut s, &rows, paused, now);
        }
        let plans = {
            let s = sch.state()?;
            let threshold = ms(s.threshold.unwrap_or(MISFIRE_THRESHOLD));
            s.slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| {
                    slot.next <= now && !slot.paused && !s.paused.contains(slot.job)
                })
                .map(|(i, slot)| (i, self.plan(slot, now, threshold)))
                .collect::<Vec<_>>()
        };
        let mut queued = 0;
        for (i, plan) in plans {
            let (job, id, from) = {
                let s = sch.state()?;
                let slot = &s.slots[i];
                (slot.job, slot.id.clone(), slot.next)
            };
            let count = plan.runs.len();
            let result = store
                .fire(job, &id, from, plan.to, plan.runs, plan.skipped)
                .await;
            let mut s = sch.state()?;
            match result {
                Ok(None) => {
                    s.slots[i].next = plan.to;
                    queued += count;
                }
                Ok(Some(stored)) => s.slots[i].next = stored,
                Err(e) => {
                    tracing::error!(
                        job,
                        trigger = &*id,
                        "queueing the ticks of trigger `{id}` failed: {e:?}"
                    )
                }
            }
        }
        if queued > 0 {
            self.wake_workers();
        }
        let prune = {
            let s = sch.state()?;
            (now - s.pruned >= ms(PRUNE_EVERY)).then_some(s.retention)
        };
        if let Some(r) = prune {
            sch.state()?.pruned = now;
            let before = now.saturating_sub(i64::from(r.days) * 86_400_000);
            if let Err(e) = store
                .prune_history(before, r.max_rows, PRUNE_BATCH)
                .await
            {
                tracing::warn!("pruning the job history failed: {e:?}");
            }
        }
        Ok(queued)
    }

    fn plan(&self, slot: &Slot, now: i64, threshold: i64) -> Plan {
        let mut runs = Vec::new();
        let mut missed = 0u32;
        let mut last_missed = None;
        let mut t = slot.next;
        let mut scanned = 0;
        let cap = match slot.trigger.misfire {
            Misfire::RunAll { max } => max.min(MAX_CATCH_UP),
            _ => 0,
        };
        while t <= now && scanned < MAX_MISFIRE_SCAN {
            scanned += 1;
            let excluded = slot
                .calendar
                .as_ref()
                .is_some_and(|c| c.excludes(crate::time::DateTime::from_unix_ms(t), slot.tz));
            if !excluded {
                if now - t <= threshold {
                    runs.push(self.tick_run(slot, t));
                } else {
                    missed += 1;
                    last_missed = Some(t);
                    if missed <= cap {
                        runs.push(self.tick_run(slot, t));
                    }
                }
            }
            t = slot.kind.next(t, &slot.zone);
        }
        if t <= now {
            t = slot.kind.next(now, &slot.zone);
        }
        let skipped_count = match slot.trigger.misfire {
            Misfire::RunOnce => {
                if let Some(at) = last_missed {
                    runs.insert(0, self.tick_run(slot, at));
                }
                0
            }
            Misfire::RunAll { .. } => missed.saturating_sub(cap),
            Misfire::Skip => missed,
        };
        let skipped = (skipped_count > 0).then(|| RunRow {
            id: crate::jobs::random_id(),
            run_id: String::new(),
            job: slot.job.into(),
            trigger_id: Some(slot.id.to_string()),
            scheduled_ms: last_missed,
            started_ms: now,
            ended_ms: now,
            attempt: skipped_count,
            outcome: "skipped".into(),
            error_code: None,
            worker: self.worker_id().into(),
            started_by: None,
        });
        Plan {
            to: t,
            runs,
            skipped,
        }
    }

    fn tick_run(&self, slot: &Slot, tick: i64) -> NewJob {
        let mut job = self
            .new_job(slot.job, slot.trigger.payload.clone(), Duration::ZERO)
            .unwrap_or_default();
        job.run_at_ms = tick;
        job.trigger_id = Some(slot.id.to_string());
        job.tick_key = Some(format!("{}:{}:{tick}", slot.job, slot.id));
        job.scheduled_ms = Some(tick);
        job.priority = slot.trigger.priority;
        job.timeout_ms = slot.trigger.timeout.map(ms);
        job
    }

    fn slot_of(&self, job: &str, trigger: Option<&str>) -> Result<()> {
        let s = self.scheduler().state()?;
        let known = match trigger {
            None => self.names().contains(&job),
            Some(id) => s
                .pending
                .iter()
                .any(|(j, t)| j == job && t.id.as_deref() == Some(id)),
        };
        if known { Ok(()) } else { Err(Error::NotFound) }
    }

    /// Pauses a job: its triggers queue nothing and no worker claims its queued runs, on every
    /// server that shares the store, until [`Jobs::resume`].
    pub async fn pause(&self, job: &str) -> Result<()> {
        self.set_paused(job, None, true).await
    }

    /// Pauses one trigger of a job; the job's other triggers and queued runs go on.
    pub async fn pause_trigger(&self, job: &str, trigger: &str) -> Result<()> {
        self.set_paused(job, Some(trigger), true)
            .await
    }

    /// Resumes a paused job; `how` says what happens to the ticks of the paused time.
    pub async fn resume(&self, job: &str, how: Resume) -> Result<()> {
        self.set_paused(job, None, false)
            .await?;
        self.after_resume(job, None, how).await
    }

    /// Resumes a paused trigger.
    pub async fn resume_trigger(&self, job: &str, trigger: &str, how: Resume) -> Result<()> {
        self.set_paused(job, Some(trigger), false)
            .await?;
        self.after_resume(job, Some(trigger), how)
            .await
    }

    async fn set_paused(&self, job: &str, trigger: Option<&str>, paused: bool) -> Result<()> {
        self.slot_of(job, trigger)?;
        self.compile();
        let now = self.scheduler().now();
        let store = self.store_handle();
        if let Some(id) = trigger {
            let row = {
                let s = self.scheduler().state()?;
                s.slots
                    .iter()
                    .find(|x| x.job == job && &*x.id == id)
                    .map(|x| row_of(x, x.kind.next(now, &x.zone)))
            };
            if let Some(row) = row {
                store.add_trigger(row).await?;
            }
        }
        store
            .set_paused(job, trigger, paused, now)
            .await?;
        let mut s = self.scheduler().state()?;
        match trigger {
            None if paused => {
                s.paused.insert(job.into());
            }
            None => {
                s.paused.remove(job);
            }
            Some(id) => {
                for slot in s
                    .slots
                    .iter_mut()
                    .filter(|x| x.job == job && &*x.id == id)
                {
                    slot.paused = paused;
                }
            }
        }
        Ok(())
    }

    async fn after_resume(&self, job: &str, trigger: Option<&str>, how: Resume) -> Result<()> {
        if how == Resume::Misfire {
            return Ok(());
        }
        let now = self.scheduler().now();
        let moves: Vec<(Arc<str>, i64)> = {
            let s = self.scheduler().state()?;
            s.slots
                .iter()
                .filter(|x| x.job == job && trigger.is_none_or(|id| &*x.id == id))
                .map(|x| (x.id.clone(), x.kind.next(now, &x.zone)))
                .collect()
        };
        let store = self.store_handle();
        for (id, next) in &moves {
            store
                .move_trigger(job, id, *next)
                .await?;
        }
        let mut s = self.scheduler().state()?;
        for (id, next) in moves {
            if let Some(slot) = s
                .slots
                .iter_mut()
                .find(|x| x.job == job && x.id == id)
            {
                slot.next = next;
            }
        }
        Ok(())
    }

    /// Queues a run of `job` now with `payload`, recorded in history as started by the caller
    /// (trigger `manual`). The run still runs as `job:<name>`, never with the caller's
    /// permissions. A paused job is refused with `Error::Conflict`; resume it first.
    pub async fn run_now(
        &self,
        ctx: &crate::Ctx,
        job: &str,
        payload: impl Into<String>,
    ) -> Result<String> {
        if self.scheduler().is_paused(job) {
            return Err(Error::Conflict);
        }
        let mut new = self.new_job(job, payload.into(), Duration::ZERO)?;
        new.trigger_id = Some("manual".into());
        new.started_by = Some(
            ctx.identity()
                .user
                .clone()
                .unwrap_or_else(|| "anonymous".into()),
        );
        let id = self.store_handle().enqueue(new).await?;
        self.wake_workers();
        Ok(id)
    }

    /// Run history, newest first: `limit` rows (at most [`crate::MAX_PAGE_SIZE`]) after skipping
    /// `offset`, of one job or all.
    pub async fn history(
        &self,
        job: Option<&str>,
        offset: u64,
        limit: usize,
    ) -> Result<Vec<RunRow>> {
        self.store_handle()
            .history(job, offset, limit.min(crate::MAX_PAGE_SIZE as usize))
            .await
    }

    /// The triggers of `job` with their next three ticks.
    pub fn triggers(&self, job: &str) -> Vec<TriggerStatus> {
        self.compile();
        let Ok(s) = self.scheduler().state() else {
            return Vec::new();
        };
        let now = self.scheduler().now();
        s.slots
            .iter()
            .filter(|x| x.job == job)
            .map(|x| {
                let mut next = Vec::with_capacity(3);
                let mut t = if x.next == i64::MAX || !s.loaded {
                    x.kind.next(now, &x.zone)
                } else {
                    x.next
                };
                while next.len() < 3 && t != i64::MAX {
                    next.push(t);
                    t = x.kind.next(t, &x.zone);
                }
                TriggerStatus {
                    id: x.id.to_string(),
                    expr: x.trigger.source.clone(),
                    time_zone: x.tz.name().into(),
                    calendar: x.trigger.calendar.clone(),
                    paused: x.paused,
                    next_ms: next,
                }
            })
            .collect()
    }

    /// Whether a job is paused, as this server last read it.
    pub fn is_paused(&self, job: &str) -> bool {
        self.scheduler().is_paused(job)
    }
}

struct Plan {
    to: i64,
    runs: Vec<NewJob>,
    skipped: Option<RunRow>,
}

fn ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

fn zone_of(name: &str) -> std::result::Result<crate::TimeZone, ()> {
    crate::TimeZone::named(name).ok_or(())
}

fn row_of(slot: &Slot, next: i64) -> TriggerRow {
    TriggerRow {
        job: slot.job.into(),
        id: slot.id.to_string(),
        expr: slot.trigger.source.clone(),
        time_zone: slot.tz.name().into(),
        calendar: slot.trigger.calendar.clone(),
        misfire: slot.trigger.misfire.code().into(),
        misfire_max: match slot.trigger.misfire {
            Misfire::RunAll { max } => Some(max),
            _ => None,
        },
        priority: slot.trigger.priority,
        paused: false,
        next_ms: next,
    }
}

fn apply_rows(s: &mut State, rows: &[TriggerRow], paused: Vec<String>, now: i64) {
    for slot in &mut s.slots {
        if let Some(r) = rows
            .iter()
            .find(|r| r.job == slot.job && r.id == *slot.id)
        {
            slot.next = r.next_ms;
            slot.paused = r.paused;
        }
    }
    s.paused = paused.into_iter().collect();
    s.refreshed = now;
}
