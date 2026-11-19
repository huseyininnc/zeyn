# Metronome: a scheduler on top of Zeyn jobs

Status: implemented behind the Cargo feature `metronome`. This page is the guide; the sections
on prior art and the reasons for each choice are kept from the design review, and "Not built
yet" lists what the design proposed that is still open.

Metronome is the name the owner chose over Clockwork and Chronos:

- **Clockwork**: clear, but used by several Rust and Ruby scheduling crates and gems, which makes
  search and crate names crowded.
- **Chronos**: well known, but taken by Mesos Chronos, a Java scheduler and many crates. It also
  suggests a time library rather than a scheduler.
- **Metronome**: a steady beat, short, and free as a module name inside `zeyn`. It does not need
  its own crate name because it ships as a feature of `zeyn`.

The feature is `metronome`, the module `zeyn::metronome`, and the main types `Trigger`,
`Calendar`, `Misfire` and `Cron`.

## Where we start

Zeyn already has background jobs (`docs/enterprise.md`, sections "Background jobs" and "Durable
jobs", code in `crates/zeyn/src/jobs.rs` and `crates/zeyn/src/jobs/`):

- `App::job(name, schedule, fn)` and `App::queued_job(name, schedule, fn)` register jobs.
- `Schedule::every(d)` and `Schedule::manual()`, with `.retry(times, first_delay)` and a doubling
  backoff capped at `MAX_BACKOFF_MS` (ten minutes).
- `job_concurrency` limits how many jobs run at once (four by default).
- `JobStore` (`enqueue`, `schedule`, `claim` with a lease, `heartbeat`, `complete`, `fail`) with
  `MemoryJobStore` (at most 10,000 runs) and `SqlJobStore` for PostgreSQL and Oracle in the
  `zeyn_jobs` table. Workers poll every `JOB_POLL` (one second); leases last `JOB_LEASE` (60
  seconds) and are renewed every 20 seconds.
- A queued run carries a payload of at most `MAX_JOB_PAYLOAD` (4000 bytes).
- `GET /_zeyn/jobs` shows status to users with `jobs_permission`; error details only go to the
  server log.

What is missing for business schedules: wall clock schedules ("every weekday at 07:30 Istanbul
time"), holidays, a defined answer to "the server was down at 07:30, what now", pausing, a run
history people can read, and a screen for operators. Metronome adds these without a second job
system: a trigger only decides *when* a run is queued; the existing claim, lease, retry and
concurrency code still runs it.

## Goals

1. Cron schedules with seconds and a time zone, with defined behavior around daylight saving
   time changes.
2. Several triggers per job, each with its own payload, priority and misfire policy.
3. Calendars that exclude days or time ranges (holidays, maintenance windows).
4. Misfire policies: run once, run all missed (capped), skip.
5. Pause and resume per job and per trigger; run now with a payload.
6. Priorities between due runs, a timeout per run, and simple job chains (run B when A succeeds).
7. Exactly one run per trigger time across all servers that share a store.
8. A run history with retention, readable in a Zeyn screen and through `/_zeyn/jobs`.
9. An operations screen built from existing components (DataGrid, toolbar actions, topics).
10. Fixed memory: nothing grows with the number of past runs or with how long a server was down.

## Non goals

- **A workflow engine.** Durable, replayed, multi step code in the style of Temporal is a
  different product. Zeyn already has approval workflows (`crate::workflow`) for human steps.
  Chains here are "on success, enqueue that job", nothing more.
- **Arbitrary code from the database.** Jobs are Rust functions registered at startup. The store
  holds triggers and runs, never code or job types that the server does not know.
- **Distributed locks for user code.** Metronome guarantees one run per trigger time. A job that
  must never overlap another job still guards itself, as today.
- **Sub second precision.** Cron has a one second resolution; workers poll every second. A run
  starts within about one poll interval of its time on a healthy server.
- **A message broker.** No Kafka or Redis queue is required. Redis stays optional, as it is today.
- **Editing schedules as free text in production by default.** Triggers come from code; changing
  them from the screen is a separate, opt in permission (see "Security").

## What others do, and what we take

| | Quartz.NET | Hangfire | sidekiq-cron | Temporal style durability |
|---|---|---|---|---|
| Schedule model | Jobs and triggers are separate; many triggers per job; cron with seconds; calendars | Recurring jobs with a cron string and a time zone | Cron string per job, minute resolution | Schedules that start workflows; code is replayed from history |
| Misfires | Per trigger instruction (fire now, do nothing, fire all missed) | Recurring jobs skip missed runs by default, `MisfireHandlingMode` in newer versions | Skips; one run per tick | Catch up window and overlap policy per schedule |
| Clustering | Row lock on a `LOCKS` table, `QRTZ_FIRED_TRIGGERS` | Distributed lock in the storage | Redis `SET NX` per tick | A central service |
| History | Listeners, no history table by default | Full history and a dashboard | None | Full event history |
| Ops UI | None built in | Dashboard with retry, delete, trigger | sidekiq web tab | Web UI |

What we adopt:

- **From Quartz.NET**: the split between a job (what runs) and its triggers (when), several
  triggers per job, cron with seconds, calendars as exclusion rules, misfire policies per trigger,
  pause per job and per trigger, priority as a tie breaker between triggers due at the same time.
- **From Hangfire**: a time zone per recurring job, a readable run history with retention, and an
  operator screen with "run now", "retry" and "delete" behind permissions.
- **From sidekiq-cron**: the simplest correct way to get one run per tick: a unique key made of
  the trigger id and the scheduled time. A second server that tries to queue the same tick fails
  the insert and moves on. No cluster lock is needed.
- **From Temporal**: the overlap policy idea (skip, queue one, allow) and a bounded catch up
  window, so a server that was down for a week does not start 10,000 runs.

What we leave out:

- Quartz.NET's `LOCKS` table and its trigger state machine (`WAITING`, `ACQUIRED`, `BLOCKED`,
  ...). The existing lease plus the unique tick key give the same guarantee with fewer moving
  parts.
- Quartz.NET's `JobDataMap` that changes between runs (stateful jobs). State lives in the
  application's own tables.
- Hangfire's storage of serialized method calls. Zeyn stores a job name and a text payload.
- Temporal's replay of code from an event history.
- A second scheduling thread per job or a timer per trigger. One loop computes the next due time
  for all triggers.

## Registering triggers

```rust
use zeyn::metronome::{Calendar, Misfire, Retention, Trigger, Weekday};

App::new("Billing")
    .job_store(SqlJobStore::new(db.clone()))
    .calendar(
        "tr-holidays",
        Calendar::days(["2026-01-01", "2026-04-23", "2026-05-01"])
            .weekly_off([Weekday::Saturday, Weekday::Sunday]),
    )
    .queued_job("close_day", Schedule::manual().retry(3, Duration::from_secs(30)), close_day)
    .trigger(
        "close_day",
        Trigger::cron("0 30 23 * * *")
            .id("nightly")
            .time_zone("Europe/Istanbul")
            .calendar("tr-holidays")
            .misfire(Misfire::RunOnce)
            .timeout(Duration::from_secs(900)),
    )
    .trigger(
        "close_day",
        Trigger::cron("0 0 12 L * ?")
            .id("month_end")
            .payload("month")
            .priority(10)
            .misfire(Misfire::RunAll { max: 3 }),
    )
    .chain("close_day", "send_reports")
    .job_history(Retention::days(30).max_rows(100_000))
    .feature(zeyn::metronome::ops())
```

- `Trigger::cron(expr)` takes six fields (seconds first) or seven (with the year), in the Quartz
  dialect: `*`, lists, ranges (`22-2` wraps), steps, `?`, `L` and `L-3` (last day, three days
  before it), `15W` and `LW` (nearest weekday), `6L` (last Friday) and `MON#1` (first Monday).
  Weekdays are names or `1` to `7` with `1` for Sunday, as in Quartz; `0` is refused with a
  message that says so. One of the two day fields must be `*` or `?`. Five field expressions are
  refused with a message that shows the six field form, so there is one dialect. `Cron::parse`
  and `Cron::next_after` are public for tools and tests.
- `Trigger::every(d)` runs every interval (at least a second), the first time one interval after
  the trigger is first stored; `Trigger::once(at)` runs one time. `Schedule::every` keeps working
  as before and is not a trigger.
- `.time_zone(name)` takes an IANA name or a fixed offset. It is UTC unless set. Names are checked
  against the tz database that `jiff` bundles, which `zeyn` already depends on.
- `.id(name)` names the trigger (1 to 64 of `a-z`, `0-9`, `_`, `-`). Without one it is `cron:<n>`
  in registration order for that job, stable as long as the code is.
- `.payload(text)` goes to `queued_job` functions, at most `MAX_JOB_PAYLOAD` (4000 bytes).
- `.priority(i16)`: higher first, 0 unless set. A run waiting longer than `PRIORITY_AGING` (five
  minutes) is claimed as the highest priority, so none starves.
- `.timeout(d)`: the try is dropped when the time is up and fails with the error code `timeout`;
  the job's retries apply. No limit unless set.
- `App::chain(a, b)`: after a run of `a` succeeds, `b` is queued with the payload of `a`'s run
  (trigger `chain`, started by `job:a`). A failed or timed out run starts nothing.
- `.feature(zeyn::metronome::ops())` needs the feature `metronome-ops` (see "Operations screen").
- `App::trigger`, `App::calendar` and `App::chain` are checked when the app starts: a bad
  expression, an unknown time zone, job or calendar, a duplicate trigger id, a trigger that never
  runs, a chain cycle or a chain longer than `MAX_CHAIN` (8) stop `App::check` and `serve` with
  problem `Z0508`, which names the trigger and says what to change. At most `MAX_TRIGGERS`
  (1,000) triggers per app.
- A running job reads `ctx.scheduled_at()` (the tick, UTC) and `ctx.trigger_id()` (the trigger
  id, `manual` or `chain`), so a daily report run late still reports the right day.

The compiler refuses a bare string where a trigger belongs, `Misfire::RunAll` without its cap
and a timeout that is not a `Duration`; the `compile_fail` doctests on `Trigger` keep that so.

### Daylight saving time

Cron fields describe wall clock time in the trigger's time zone. When the clock changes:

- **Gap** (spring forward, 02:00 to 03:00 does not exist): a tick in the gap runs once, at the
  first instant after the gap (03:00). It is not skipped: a daily job at 02:30 still runs that
  day. This is what Quartz.NET and most operators expect.
- **Fold** (fall back, 02:00 to 03:00 happens twice): a tick in the fold runs once, at its first
  occurrence (the earlier UTC instant). When the hour field is `*` or a step, the expression
  follows the real clock instead and matches in both passes: an hourly `0 0 */1 * * *` runs every
  hour of real time through the fold.
- Every tick is stored as a UTC instant (Unix milliseconds), so servers in different zones agree.
- The tests cover Europe/Berlin, America/New_York, Australia/Lord_Howe (a 30 minute shift),
  Pacific/Apia (the day Samoa skipped in 2011) and Europe/Istanbul (no DST since 2016).

### Calendars

A calendar is an exclusion rule. A tick in an excluded day or range is not run and does not count
as a misfire.

- `Calendar::days([...])`: whole dates, read in the trigger's time zone, at most
  `MAX_CALENDAR_DAYS` (3,660), kept as a sorted list of day numbers.
- `.weekly_off([...])`: days of the week.
- `.annual([(month, day)])`: dates that repeat every year.
- `.ranges([(start, end)])`: daily time ranges such as `("23:30", "00:30")`; an end before the
  start crosses midnight.
- `.and(other)`: what either calendar excludes.

### Misfire policies

A tick is misfired when it is found more than `MISFIRE_THRESHOLD` (60 seconds,
`Jobs::set_misfire_threshold`) after its time: the servers were down, paused or saturated.

- `Misfire::RunOnce` (default): one run now for all missed ticks, scheduled for the latest one;
  then the schedule moves on.
- `Misfire::RunAll { max }`: one run per missed tick, oldest first, at most `max` (capped at
  `MAX_CATCH_UP`, 100), each with its own scheduled time. The rest are recorded as one `skipped`
  history row whose `attempt` holds the count.
- `Misfire::Skip`: no run for missed ticks; one `skipped` history row records the count.

A pass looks at no more than `MAX_MISFIRE_SCAN` (10,000) ticks of one trigger; a trigger further
behind (a schedule of every second after a long downtime) jumps to its next future tick, and the
skipped count stops at that number. Nothing grows with the length of a downtime.

### Pause, resume and run now

```rust
jobs.pause("close_day").await?;
jobs.pause_trigger("close_day", "month_end").await?;
jobs.resume("close_day", Resume::Skip).await?;
jobs.resume_trigger("close_day", "month_end", Resume::Misfire).await?;
jobs.run_now(&ctx, "close_day", "2026-11-14").await?;
```

- Pause state is kept in the store, so it holds across restarts and servers. A paused job keeps
  its queued runs, but no worker claims them and its triggers queue nothing; a paused trigger
  queues nothing. Other servers see a pause within `TRIGGER_REFRESH` (30 seconds).
- `Resume::Skip` moves the triggers to their next future tick; `Resume::Misfire` applies each
  trigger's misfire policy to the paused time.
- `run_now(ctx, job, payload)` queues a run with the trigger id `manual` and records the caller
  in history as `started_by`. The run still runs as `job:<name>` with no permissions, so "run
  now" cannot raise privileges. A paused job answers `Error::Conflict`; resume it first.

### Status

`JobStatus` gains `paused` and `triggers`: each `TriggerStatus` has the id, the expression in its
source text, the time zone, the calendar, the pause flag and the next three ticks.
`Jobs::triggers(job)`, `Jobs::is_paused(job)` and `Jobs::history(job, offset, limit)` read them in
code. These are plain values built on request, not kept in memory.

## How a tick becomes a run

1. One scheduler loop per server (a task next to the job worker) keeps, for each trigger, only its
   next tick: one `i64`.
2. Every `JOB_POLL` it takes the triggers whose next tick is due, applies calendars and the
   misfire policy, and asks the store to `fire`: in one transaction, move the trigger's
   `next_ms` from the value it read to the next one with a compare and set
   (`UPDATE ... SET next_ms = ? WHERE job = ? AND id = ? AND next_ms = ?`) and insert the runs.
3. When two servers fire the same tick, one update wins; the other changes no row, inserts
   nothing and takes the stored next tick. That is the whole cross server guarantee: exactly one
   run per trigger time, with no lock table and no leader election. Each run also carries
   `tick_key` (`job:trigger:tick_ms`) under a unique index as a second guard.
4. Workers claim runs as before. PostgreSQL orders the claim by
   `CASE WHEN run_at < aging THEN 32767 ELSE priority END DESC, run_at`. Oracle cannot order a
   locking select; there the claim reads the highest due rank without a lock and then takes that
   band first (`AND rank >= ?`), which keeps the order between bands and leaves it open inside
   one, as before.
5. `Jobs::run_due()` runs one pass by hand; tests drive it with a `FakeClock`
   (`Jobs::set_clock`), which moves only when the test says so.

`MemoryJobStore` implements the same methods under its lock, so the same code runs in one
process, and two schedulers on one memory store get the same guarantee.

## Store and SQL schema

`JobStore` gains `add_trigger`, `triggers`, `fire`, `move_trigger`, `set_paused`, `paused_jobs`,
`history` and `prune_history`. They have default implementations that return an error, so a
custom store still compiles; `MemoryJobStore` and `SqlJobStore` implement them all. `NewJob` and
`ClaimedJob` gain `trigger_id`, `scheduled_ms`, `timeout_ms` and `started_by` (and `NewJob` the
`tick_key` and `priority`, `ClaimedJob` the `started_ms`); both implement `Default`.

With the feature on, `create_job_tables(dialect, prefix)` returns schema version 2:

| Column of `zeyn_jobs` | PostgreSQL | Oracle | Meaning |
|---|---|---|---|
| `trigger_id` | `VARCHAR(64)` | `VARCHAR2(64)` | `manual`, `chain`, the trigger id, or null |
| `tick_key` | `VARCHAR(160)` | `VARCHAR2(160)` | unique per trigger time; null for other runs |
| `scheduled_ms` | `BIGINT` | `NUMBER(19)` | the tick, for `ctx.scheduled_at()` |
| `priority` | `BIGINT DEFAULT 0 NOT NULL` | `NUMBER(5) DEFAULT 0 NOT NULL` | claim order |
| `timeout_ms` | `BIGINT` | `NUMBER(19)` | per run timeout |
| `started_ms` | `BIGINT` | `NUMBER(19)` | start of the current try |
| `started_by` | `VARCHAR(128)` | `VARCHAR2(128)` | the caller of a manual run |

and next to it `zeyn_jobs_tick` (unique index on `tick_key`; many nulls are allowed on both
databases), `zeyn_job_triggers` (primary key `(job, id)`: expression, zone, calendar, misfire,
priority, `paused`, `next_ms`, `updated_ms`), `zeyn_job_pauses` (one row per paused job),
`zeyn_job_runs` (history, indexed on `(job, ended_ms)` and `ended_ms`) and `zeyn_job_schema`
(one row, version 2). Whole numbers are `BIGINT` on PostgreSQL, the type the data layer reads.
Oracle uses `VARCHAR2`, `NUMBER(5)`, `NUMBER(10)` and `NUMBER(19)`, with `DEFAULT` before
`NOT NULL`; every name takes the table prefix and stays within 30 bytes with a prefix of up to
8 bytes, which a test checks. An Oracle claim takes at most 1,000 runs, the size of an `IN` list.

`migrate_job_tables(dialect, prefix)` returns the statements from version 1 (a table made without
the feature) to version 2: `ALTER TABLE ... ADD` for each column, all nullable or with a default,
so PostgreSQL 11 and later and Oracle 11g change only metadata, then the new tables. Review them
and put them in a migration. A version 1 server keeps working against a version 2 schema; it
ignores the new columns.

## Run history and retention

- One row per finished try, written in the same transaction as `complete` or `fail` on
  `SqlJobStore`, so history does not miss a run, at the cost of one insert per try.
- Rows hold the job, trigger, scheduled time, start, end, try, outcome (`succeeded`, `failed`,
  `timeout` or `skipped`), error code, worker and `started_by`. Never the payload, never the
  error text; details stay in the server log.
- Retention: `App::job_history(Retention::days(30).max_rows(100_000))`, which is the default.
  Every `PRUNE_EVERY` (ten minutes) each server deletes at most `PRUNE_BATCH` (1,000) rows that
  are older than the retention or beyond the newest `max_rows`, so it never holds a long lock.
  Deletes are idempotent, so servers need no coordination for it.
- `MemoryJobStore` keeps history in a ring of `MEMORY_HISTORY_CAPACITY` (1,000) rows.
- `GET /_zeyn/jobs/runs` returns history as JSON (`?job=`, `?offset=`, `?limit=` up to
  `MAX_PAGE_SIZE`) to users with `jobs_permission`, like `GET /_zeyn/jobs`.

## Operations screen

With the feature `metronome-ops` (which turns on `metronome`),
`App::feature(zeyn::metronome::ops())` mounts the screen at `/ops/jobs`, built only from existing components, with a menu entry under
"Operations". It is a feature of its own because its three DataGrids are most of the binary
size; apps that only schedule leave it off, and its texts live in `locales/ops/` so builds
without it carry none of them.

- **Jobs grid**: name, state, paused, next run in the viewer's time zone, last error code, runs
  and failures. Row actions: run now, pause, resume (next tick) and resume with catch up (misfire
  policy).
- **Triggers grid**: job, id, expression, time zone, calendar, paused and the next three ticks.
  Row actions: pause and resume a trigger.
- **Runs grid**: job, trigger, scheduled time, start, duration, try, outcome, error code, worker
  and started by, paged on the server.
- **Live status**: the grids listen to the topic `zeyn.jobs`. With the feature `realtime`,
  workers notify it when a run ends, at most once a second per server; without it the grids
  refresh with the grid's refresh control. Only the topic name travels; the refresh goes through
  the normal endpoints with their permission checks.
- Texts come from the framework locale files in every built in language; the screen uses theme
  tokens and the standard grid keyboard model and adds no browser module.

## Security

- **Permissions**: `zeyn.jobs.read` (`metronome::JOBS_READ`) opens the screen and its grids,
  `zeyn.jobs.run` (`JOBS_RUN`) runs a job now, `zeyn.jobs.admin` (`JOBS_ADMIN`) pauses and
  resumes. Each action checks its permission on the server for every call, including calls the
  screen does not offer; row action buttons only show to users who may use them. The JSON
  endpoints keep `jobs_permission`.
- **Audit**: run now, pause and resume are `audit` actions with the caller's identity.
- **CSRF and rate limit**: row actions go through the grid endpoint (`check_csrf`, write rate
  limit).
- **Payloads**: `MAX_JOB_PAYLOAD` still applies. Payloads are never written to history, logs, the
  status endpoint or the screen. The recommended pattern stays "the payload is an id; the job
  reads the record".
- **Identity**: scheduled, chained and manual runs all run as `job:<name>` with no permissions.
- **Tenants**: a trigger belongs to the app, not a tenant; a job that works per tenant loops over
  tenants itself. The live notification goes to the tenant-less topic.

## Performance and memory

| Item | Budget | Measured |
|---|---|---|
| Memory per trigger in the scheduler | at most 256 bytes | about 200 bytes; the compiled `Cron` is 80 bytes |
| Triggers per app | `MAX_TRIGGERS` 1,000 | checked at startup |
| Next tick computation | under 2 microseconds, no allocation | 0.17 to 1.3 microseconds in a release build (0.3 for a daily time in Europe/Berlin, 1.3 for `L` and `#` days), no allocation |
| Scheduler pass when nothing is due | no database query | one comparison per trigger; triggers and pauses are read from the store every 30 seconds |
| Misfire catch up | bounded, never a list of all missed ticks | at most 10,000 ticks looked at and 100 runs queued per trigger and pass; a year of downtime on a trigger of every second takes under 60 ms in a release build, with 100 runs queued |
| History in the memory store | ring of 1,000 rows | as budgeted |
| Live notifications | at most one per second per server | as budgeted |
| Binary size of the showcase (release) | under 60 KB with the feature on | `metronome`: +238,592 bytes (233 KB) for the scheduler, store and three triggers; `metronome-ops`: +904,704 bytes (884 KB) with the operations screen, whose three DataGrids are most of it, so the screen is a feature of its own. Without either feature the binary is the size it was before (28,018,688 bytes against 28,023,296 on `dev`, within noise) |

## Not built yet

These parts of the design are open; each would be a change of its own.

- `Trigger::overlap` (skip, queue one, allow). Today ticks queue as they come and
  `job_concurrency` bounds what runs at once; a job that must not overlap itself guards itself.
- `Calendar::from_store` with the table `zeyn_job_calendars`, to edit holidays from a screen.
- Editing triggers from the screen (with `MIN_TRIGGER_INTERVAL`), and a payload form for "run
  now"; the screen runs a job with an empty payload.
- `RunNow::EvenIfPaused`.
- A startup check of `zeyn_job_schema` that refuses an old schema, `zeyn jobs migrate --print`
  and a `zeyn check` warning for a store without the migration.
- History rows for runs whose lease was lost (`lost`).
- Pruning under a short lease on one server; today every server prunes, which is safe because
  deletes are idempotent.

## Decisions

1. The name: Metronome.
2. Triggers come from code only; the screen pauses, resumes and runs, but does not edit.
3. Retention defaults to 30 days and 100,000 rows, set per app with `App::job_history`.
4. Five field cron is refused with a message that shows the six field form.
