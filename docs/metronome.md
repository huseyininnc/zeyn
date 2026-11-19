# Metronome: a scheduler on top of Zeyn jobs

Status: design proposal for review. Nothing in this document is implemented yet; implementation
starts after the owner approves it.

Metronome is a working name. Two other names were considered:

- **Clockwork**: clear, but it is used by several Rust and Ruby scheduling crates and gems, which
  makes search and crate names crowded.
- **Chronos**: well known, but taken by Mesos Chronos, a Java scheduler and many crates. It also
  suggests a time library rather than a scheduler.
- **Metronome**: a steady beat, short, and free as a module name inside `zeyn`. It does not need
  its own crate name because it ships as a feature of `zeyn`.

The owner picks the final name. The code below uses `metronome` for the feature and module and
`Trigger`, `Calendar` and `Misfire` for the types, so a rename touches one feature name and one
module path.

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

## Proposed API

### Registering triggers

```rust
use zeyn::metronome::{Calendar, Cron, Misfire, Overlap, Trigger};

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
        Trigger::cron("0 0 12 L * *")
            .id("month_end")
            .payload("month")
            .priority(10)
            .misfire(Misfire::RunAll { max: 3 }),
    )
    .chain("close_day", "send_reports")
```

- `Trigger::cron(expr)` takes six fields (seconds first) or seven (with year), in the Quartz
  dialect: `*`, lists, ranges, steps, `?`, `L` (last day), `W` (nearest weekday) and `#` (nth
  weekday of the month). Five field expressions are rejected with a message that says to add the
  seconds field, so there is one dialect.
- `Trigger::every(d)` keeps today's interval behavior and is what `Schedule::every` becomes
  internally. `Trigger::once(at)` runs one time.
- `.time_zone(name)` takes an IANA name. The default is the app's time zone setting
  (`docs/operations.md`), and UTC when none is set. Names are checked at startup against the
  bundled tz database of `jiff`, which `zeyn` already depends on, so no new crate is needed.
- `.id(name)` names the trigger (1 to 64 bytes, `[a-z0-9_-]`). Without it the id is `cron:<n>`
  in registration order, which is stable as long as the code is.
- `.payload(text)` is passed to `queued_job` functions, at most `MAX_JOB_PAYLOAD`.
- `.priority(i16)`: higher first, default 0. Priority orders due runs within one claim; it never
  starves: a run waiting longer than `PRIORITY_AGING` (five minutes) is treated as top priority.
- `.timeout(d)`: the run's future is dropped when the time is up and the try fails with the error
  code `timeout`. The default is no timeout, as today. A timeout longer than the lease keeps the
  heartbeat going, as a normal run does.
- `.overlap(Overlap::Skip | Overlap::QueueOne | Overlap::Allow)`: what a tick does while the
  previous run of the same trigger still runs. The default is `QueueOne`, which matches today's
  "a job triggered while it runs runs once more afterwards".
- `.chain(a, b)`: after a run of `a` succeeds, `b` is queued with the payload of `a`'s run. Chains
  are checked at startup: both jobs must exist and the chain graph has no cycle. A chain of at
  most `MAX_CHAIN` (8) jobs keeps a mistake from spinning.
- `App::trigger` and `App::calendar` fail at startup (not at the first tick) on a bad cron string,
  an unknown time zone, an unknown calendar or job, or a duplicate trigger id. Each message names
  the trigger and says what to change, written for an AI agent to act on.

### Daylight saving time

Cron fields describe wall clock time in the trigger's time zone. When the clock changes:

- **Gap** (spring forward, 03:00 to 04:00 does not exist): a tick in the gap runs once, at the
  first instant after the gap (04:00). It is not skipped: a daily job at 03:30 still runs that
  day. This is what Quartz.NET and most operators expect.
- **Fold** (fall back, 02:00 to 03:00 happens twice): a tick in the fold runs once, at its first
  occurrence (the earlier UTC instant). Second and minute patterns that match many times an hour
  (`*/10 * * * * *`) run in both passes, because they mean "every ten seconds" rather than "at
  this wall clock time". The rule: a field written as `*` or a step in the hour field follows the
  real clock; a fixed hour follows the wall clock once.
- Every computed tick is stored as a UTC instant (Unix milliseconds), so servers in different
  zones agree.
- The tests cover Europe/Istanbul (no DST since 2016), Europe/Berlin, America/New_York,
  Australia/Lord_Howe (a 30 minute shift) and a zone with a date line change.

### Calendars

A calendar is an exclusion rule. A tick that falls in an excluded range is not run and does not
count as a misfire.

- `Calendar::days([...])`: whole dates, in the trigger's time zone.
- `.weekly_off([...])`: days of the week.
- `.ranges([(start_time, end_time)])`: daily time ranges, such as a maintenance window.
- `.annual([(month, day)])`: dates that repeat every year.
- `.and(other)`: the union of two calendars.
- `Calendar::from_store(name)`: dates kept in the database table `zeyn_job_calendars`, so
  holidays can be maintained from a screen without a deploy. The scheduler reloads them when the
  table's version changes (one `SELECT max(version)` per minute), and holds at most
  `MAX_CALENDAR_DAYS` (3660, ten years of days) per calendar.

### Misfire policies

A tick is misfired when it is found more than `MISFIRE_THRESHOLD` (default 60 seconds,
`.misfire_threshold(d)` per app) after its time: the servers were down, paused or saturated.

- `Misfire::RunOnce` (default): one run for all missed ticks, now; then the schedule moves on.
- `Misfire::RunAll { max }`: one run per missed tick, oldest first, at most `max` (capped at
  `MAX_CATCH_UP`, 100). Each run gets its scheduled time, so a daily report run late still
  reports the right day. Missed ticks past `max` are recorded as one `skipped` history row with
  the count.
- `Misfire::Skip`: no run for missed ticks; the next run is the next future tick. One `skipped`
  history row records the count.

The run's context gets `ctx.scheduled_at()` (the tick, in UTC) and `ctx.trigger_id()`, so a job
does not need to guess which day it is working for.

### Pause, resume and run now

```rust
jobs.pause("close_day").await?;
jobs.pause_trigger("close_day", "month_end").await?;
jobs.resume("close_day", Resume::Skip).await?;
jobs.run_now("close_day", "2026-11-14").await?;
```

- Pause state is kept in the store, so it holds across restarts and servers. A paused job keeps
  its queued runs but no worker claims them; a paused trigger queues no new ticks.
- `Resume::Skip` moves the trigger to its next future tick; `Resume::Misfire` applies the
  trigger's misfire policy to the paused time. The operator chooses on the screen.
- `run_now(job, payload)` is `enqueue` with the trigger id `manual` and the identity of the caller
  recorded in history. It runs even when the job is paused only if the caller passes
  `RunNow::EvenIfPaused`.

### Status types

`JobStatus` gains `paused`, `next_run` (UTC), `triggers: Vec<TriggerStatus>` and the last three
outcomes. `TriggerStatus` holds the id, the expression in its source text, the time zone, the next
three ticks (for the screen) and the pause flag. These are plain values, cheap to clone, built
from the store on request rather than kept in memory.

## How a tick becomes a run

1. One scheduler loop per server (inside the existing job worker task, no new thread) keeps, for
   each trigger, only its next tick: one `i64` per trigger.
2. Every `JOB_POLL` it takes the triggers whose next tick is due, applies calendars and the
   misfire policy, and inserts runs with `tick_key = trigger_id || ':' || tick_ms`.
3. The `tick_key` column has a unique index. When two servers insert the same tick, one insert
   wins; the other gets a unique violation, which the store treats as "already queued" and
   ignores. That is the whole cross server guarantee: at most one run per trigger time, with no
   lock table and no leader election.
4. The trigger's next tick is stored in `zeyn_job_triggers.next_ms` with a compare and set
   (`UPDATE ... SET next_ms = ? WHERE id = ? AND next_ms = ?`), so a server that was behind does
   not move it backwards.
5. Workers claim runs as today, now ordered by `priority DESC, run_at` on PostgreSQL. Oracle
   cannot order a locking select; there the claim takes the highest priority band first
   (`WHERE priority >= ?` with the highest due priority read without a lock), which keeps the
   ordering correct between bands and unordered inside one band, as it is today.

Without a store (`MemoryJobStore`), the same code runs in one process, and the unique tick key is
a `HashSet` entry that is removed when the run finishes, capped like the store.

## Store and SQL schema

### Trait changes

`JobStore` gains methods with default implementations that return an "unsupported" error, so a
custom store still compiles; `MemoryJobStore` and `SqlJobStore` implement them all.

```rust
fn upsert_trigger(&self, t: &TriggerRow) -> BoxFuture<Result<()>>;
fn advance_trigger(&self, id: &str, from_ms: i64, to_ms: i64) -> BoxFuture<Result<bool>>;
fn enqueue_tick(&self, job: NewJob, tick_key: &str) -> BoxFuture<Result<Enqueued>>;
fn set_paused(&self, target: PauseTarget<'_>, paused: bool) -> BoxFuture<Result<()>>;
fn history(&self, query: &DataQuery) -> BoxFuture<Result<Page<RunRow>>>;
fn prune_history(&self, before_ms: i64, limit: usize) -> BoxFuture<Result<u64>>;
```

`Enqueued` is `Queued(id)` or `Duplicate`. `history` takes a `DataQuery`, so the run history is a
normal DataGrid source with sorting, filters and cursor paging, and the page size limits of
`DataQuery` apply.

### Tables

Changes to `zeyn_jobs` (new columns are nullable or have defaults, so the migration is online):

| Column | PostgreSQL | Oracle | Meaning |
|---|---|---|---|
| `trigger_id` | `VARCHAR(64)` | `VARCHAR2(64)` | `manual`, `every`, or the trigger id |
| `tick_key` | `VARCHAR(100)` | `VARCHAR2(100)` | unique per trigger time; null for manual runs |
| `scheduled_ms` | `BIGINT` | `NUMBER(19)` | the tick, for `ctx.scheduled_at()` |
| `priority` | `SMALLINT DEFAULT 0` | `NUMBER(5) DEFAULT 0` | claim order |
| `timeout_ms` | `BIGINT` | `NUMBER(19)` | per run timeout |
| `started_ms` | `BIGINT` | `NUMBER(19)` | first claim of the current try |

New index: `CREATE UNIQUE INDEX zeyn_jobs_tick ON zeyn_jobs (tick_key)`. PostgreSQL and Oracle
both allow many nulls in a unique index on one nullable column, so manual runs are not affected.
The due index becomes `(state, priority, run_at)`.

New table `zeyn_job_triggers`:

```sql
CREATE TABLE zeyn_job_triggers (
  id VARCHAR(64) NOT NULL,
  job VARCHAR(64) NOT NULL,
  expr VARCHAR(200) NOT NULL,
  time_zone VARCHAR(64) NOT NULL,
  calendar VARCHAR(64),
  misfire VARCHAR(16) NOT NULL,
  misfire_max INTEGER,
  priority SMALLINT NOT NULL DEFAULT 0,
  paused SMALLINT NOT NULL DEFAULT 0,
  next_ms BIGINT NOT NULL,
  updated_ms BIGINT NOT NULL,
  PRIMARY KEY (job, id)
)
```

New table `zeyn_job_pauses` (`job VARCHAR(64) PRIMARY KEY, paused_by VARCHAR(128), paused_ms
BIGINT`), so pausing a job is one row and does not touch its triggers.

New table `zeyn_job_runs` for history (one row per finished try):

```sql
CREATE TABLE zeyn_job_runs (
  id VARCHAR(80) NOT NULL PRIMARY KEY,
  run_id VARCHAR(80) NOT NULL,
  job VARCHAR(64) NOT NULL,
  trigger_id VARCHAR(64),
  scheduled_ms BIGINT,
  started_ms BIGINT NOT NULL,
  ended_ms BIGINT NOT NULL,
  attempt INTEGER NOT NULL,
  outcome VARCHAR(16) NOT NULL,
  error_code VARCHAR(64),
  worker VARCHAR(64) NOT NULL,
  started_by VARCHAR(128)
)
CREATE INDEX zeyn_job_runs_job ON zeyn_job_runs (job, ended_ms)
CREATE INDEX zeyn_job_runs_ended ON zeyn_job_runs (ended_ms)
```

`outcome` is `succeeded`, `failed`, `timeout`, `lost` (lease ended) or `skipped` (misfire
policy). The payload is not copied to history (see "Security").

New table `zeyn_job_calendars` (`calendar VARCHAR(64), day DATE, version BIGINT`, primary key
`(calendar, day)`) for `Calendar::from_store`.

Oracle uses `VARCHAR2`, `NUMBER(5)`, `NUMBER(10)` and `NUMBER(19)` as in today's
`create_job_tables`; `DEFAULT` comes before `NOT NULL`. Every table and index name takes the
existing prefix, and Oracle names stay within 30 bytes for 11g with a prefix of up to 8 bytes
(checked by a test, as names are today).

### Migration

- `create_job_tables(dialect, prefix)` keeps returning the full schema for new apps.
- New `migrate_job_tables(dialect, prefix, from)` returns the `ALTER TABLE ... ADD` and `CREATE`
  statements from schema version 1 (today's) to version 2. Columns are added nullable or with a
  default, so no table rewrite and no long lock on PostgreSQL 11 and later; Oracle 11g adds a
  column with a default as a metadata change.
- A one row table `zeyn_job_schema (version INTEGER)` records the version. `SqlJobStore` reads it
  at startup and refuses to start with a message that names `migrate_job_tables` when the
  database is older than the code, instead of failing at the first claim.
- Rolling deploys: a version 1 server keeps working against a version 2 schema (it ignores the new
  columns and never sets `tick_key`). Mixed versions may queue a duplicate tick of an
  `every` schedule during the deploy, which is today's behavior.
- `zeyn check` reports a store without the migration; the CLI gains `zeyn jobs migrate --print`
  to print the statements for review, never to run them on its own.

## Run history and retention

- One row per finished try is written in the same transaction as `complete` or `fail`, so history
  does not miss a run and costs one extra insert per try.
- Retention: `.job_history(Retention::days(30).max_rows(100_000))`, default 30 days and 100,000
  rows per app. A pruning pass runs every ten minutes on the server that wins a short lease on the
  pseudo job `zeyn:prune`, and deletes at most 1,000 rows per pass by `ended_ms`, so it never
  holds a long lock. It is an ordinary job, so it shows up in status and history.
- `MemoryJobStore` keeps history in a ring buffer of `MEMORY_HISTORY_CAPACITY` (1,000) rows.

## Operations screen

A screen the app mounts with `.jobs_screen("/ops/jobs")`, built only from existing components:

- **Jobs grid** (`DataGrid` over `jobs.status()`): name, state, paused, next run in the viewer's
  time zone, last outcome, runs and failures in the last 24 hours. Row actions: run now (with an
  optional payload in a form), pause, resume (with the skip or misfire choice), retry the last
  failed run.
- **Triggers grid** under the selected job: id, expression, time zone, calendar, next three ticks,
  paused. Row actions: pause and resume a trigger.
- **Runs grid** (`DataGrid` over `history`): job, trigger, scheduled time, start, duration,
  attempt, outcome, error code, worker, started by. Server side paging and filters through
  `DataQuery`, so a history of a million rows costs one page of rows.
- **Live status**: the grids `.listens("zeyn.jobs")`. The worker notifies the topic `zeyn.jobs`
  through the realtime hub (feature `realtime`, `ZeynSocket::notify_tenant`) when a run starts or ends,
  at most once per second per server (coalesced), so a busy queue does not flood browsers. Without
  the `realtime` feature the grids refresh on the existing grid refresh control. Only the topic
  name travels; the refresh goes through the normal endpoints with permission checks.
- Texts come from the framework locale files (`en.json`, `tr.json`); times use the viewer's time
  zone; the screen uses theme tokens and the standard grid keyboard model, so it meets WCAG 2.2
  AA with no new browser module.

## Security

- **Permissions**: `jobs_permission` (read status and history, today's permission),
  `jobs_run_permission` (run now, retry), `jobs_admin_permission` (pause, resume, change triggers
  and calendars). All three are checked on the server for every action, including actions the
  screen hides. Without the run or admin permission configured, those actions are off.
- **Audit**: run now, pause, resume, trigger changes and calendar changes are audited actions with
  the caller's identity, through the existing audit trail.
- **CSRF and rate limit**: every action goes through the normal action endpoint (`check_csrf`),
  and run now and retry are in the write rate limit.
- **Payloads**: `MAX_JOB_PAYLOAD` (4000 bytes) still applies; payloads entered on the screen are
  validated by the job's own `params` type when the job declares one. Payloads are never written
  to history, logs, the status endpoint or the screen; the screen shows the payload length only.
  The recommended pattern stays "the payload is an id; the job reads the record", so secrets and
  personal data do not enter the job table at all.
- **Error details**: history stores the error code (`&'static str`, as today), never the error
  text, which can hold data. Details stay in the server log.
- **Identity**: a scheduled run runs as `job:<name>` with no permissions, as today. A manual run
  records `started_by` but still runs as `job:<name>`; it does not inherit the caller's
  permissions, so "run now" cannot raise privileges.
- **Cron strings from the screen** (admin only, off by default): parsed with the same parser, with
  a minimum interval of `MIN_TRIGGER_INTERVAL` (ten seconds) so a typo does not queue a run every
  second.
- **Tenants**: in multi tenant apps a trigger belongs to the app, not a tenant; a job that works
  per tenant loops over tenants itself. Tenant scoped history filters on `started_by` tenant.

## Performance budget and memory caps

| Item | Budget |
|---|---|
| Memory per trigger in the scheduler | one `TriggerState` (id, job index, compiled cron, next tick): at most 256 bytes; cron fields compiled to bit sets (60 + 60 + 24 + 31 + 12 + 7 bits) |
| Triggers per app | `MAX_TRIGGERS` 1,000, checked at startup |
| Next tick computation | under 2 microseconds per trigger on a desktop CPU, measured with a bench; no allocation |
| Scheduler loop when nothing is due | no database query: next due time is the minimum of the in memory ticks, refreshed from the store every 30 seconds for pause and trigger changes from other servers |
| Database load per poll | unchanged claim query; one insert per due tick; one history insert per finished try |
| Misfire catch up | at most `MAX_CATCH_UP` (100) runs per trigger per pass, computed lazily, never a `Vec` of all missed ticks |
| Calendars | `MAX_CALENDAR_DAYS` (3660) days per calendar, stored as a sorted `Vec<i32>` of day numbers |
| History in memory store | ring buffer of 1,000 rows |
| Live notifications | at most one `zeyn.jobs` notification per second per server |
| Binary size | behind the feature `metronome`; apps without it pay nothing. Target under 60 KB added to a release build with the feature on, measured before and after |

The implementation commit reports the measured numbers for each line above.

## Test plan

- **Fake clock**: the scheduler takes time from a `Clock` trait (`now_ms()` and `sleep_until()`),
  with `SystemClock` in production and `FakeClock` in tests, which advances only when the test
  says so. No test sleeps in real time.
- **Cron parser**: table tests for every field form, `L`, `W`, `#`, year field, rejection of five
  field expressions with the expected message, and a property test that `next(t) > t` and that
  `next` matches a brute force second by second search over random expressions and a year.
- **DST**: gap and fold rules for the zones listed above, for fixed hours and for step patterns.
- **Calendars**: excluded days, weekly off days, time ranges, union, a range that crosses
  midnight, and a calendar reload from the store.
- **Misfires**: each policy after a simulated downtime of one tick, of `max + 1` ticks and of a
  year; the history row with the skipped count; memory stays flat (no growth with downtime).
- **Cluster**: two schedulers on one `MemoryJobStore` and on PostgreSQL and Oracle (behind
  `ZEYN_TEST_PG` and `ZEYN_TEST_ORACLE`) race on the same ticks; each tick runs exactly once over
  1,000 ticks.
- **Pause and resume**: paused job not claimed, paused trigger queues nothing, both resume modes,
  pause state seen by a second scheduler within the refresh interval.
- **Timeout, priority, chains**: a run past its timeout fails with `timeout`; a higher priority
  run is claimed first; aging lifts a starved run; chains enqueue on success only and reject
  cycles at startup.
- **Migration**: a version 1 schema with rows migrates to version 2; old rows still claim; a
  version 1 server runs against a version 2 schema.
- **Security**: run now without `jobs_run_permission` is refused on the server; payloads never
  appear in the status JSON, history, screen HTML or logs (a regression test greps all four);
  CSRF on every action; manual runs keep the `job:<name>` identity.
- **Screen**: `TestApp` renders the jobs, triggers and runs grids with a nested `NodePath`; a
  browser check in the showcase with no console errors, in both themes and at phone width.
- **compile_fail doctests**: a `queued_job` trigger with a payload of the wrong `params` type, and
  a chain that names a job type the compiler can see is missing.

## Implementation tickets

Each ticket is one PR to `dev` with docs and tests.

1. **Cron and clock core.** `metronome` feature, `Cron` parser and next tick with seconds, year,
   `L`, `W`, `#`; time zones through `jiff`; DST rules; `Clock` and `FakeClock`. Acceptance: the
   parser, property and DST tests pass; the bench shows under 2 microseconds per next tick with no
   allocation; feature off adds 0 bytes.
2. **Triggers and misfires in memory.** `App::trigger`, several triggers per job, `Overlap`,
   misfire policies, `ctx.scheduled_at()` and `ctx.trigger_id()`, startup validation with agent
   oriented messages, `Schedule::every` mapped onto a trigger. Acceptance: misfire and overlap
   tests with the fake clock; existing job tests pass unchanged; memory flat after a simulated one
   year downtime.
3. **Store schema version 2 and migration.** New columns and tables for PostgreSQL and Oracle,
   `migrate_job_tables`, `zeyn_job_schema`, unique tick key, `enqueue_tick`, `advance_trigger`.
   Acceptance: cluster race test runs each of 1,000 ticks once on PostgreSQL and Oracle in Docker;
   migration test from version 1 with rows; Oracle names within 30 bytes.
4. **Calendars.** Code calendars, `Calendar::from_store`, `MAX_CALENDAR_DAYS`. Acceptance:
   calendar tests including reload; a holiday tick is neither run nor counted as a misfire.
5. **Pause, resume, run now, priority, timeout, chains.** Store backed pause, both resume modes,
   priority with aging on both dialects, per run timeout, `chain` with cycle check. Acceptance:
   the tests in "Test plan" for these items; `JobStatus` shows pause and next run.
6. **Run history and retention.** `zeyn_job_runs`, history insert in the completion transaction,
   pruning job with a short lease, memory ring buffer, `history(DataQuery)`. Acceptance: no lost
   history rows under a crash test; pruning deletes in batches of at most 1,000; payload never in
   history.
7. **Operations screen.** `jobs_screen`, three grids, row actions behind the three permissions,
   audit, live `zeyn.jobs` topic with coalescing, en and tr texts, docs page. Acceptance: TestApp
   tests for permissions and nested paths; browser check with no console errors in both themes
   and at phone width; no new browser module.
8. **Docs and CLI.** `docs/enterprise.md` sections, a docs app page in nine languages,
   `zeyn jobs migrate --print`, `zeyn check` warning, skill and template updates. Acceptance:
   docs tests and `record_translations` pass; the embedded docs are recorded.

Tickets 1 and 2 can ship without a database; 3 unlocks 4 to 6; 7 needs 5 and 6.

## Open questions for the owner

1. The final name: Metronome, Clockwork, Chronos or another.
2. Should triggers be editable from the screen at all, or only from code (simpler, and every
   change goes through review)? This document proposes code only by default with an opt in.
3. Default retention of 30 days and 100,000 rows: fine for most apps, or should it follow the
   audit retention setting?
4. Five field cron: reject (as proposed, one dialect) or accept and assume second 0.
