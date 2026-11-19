//! [`SqlJobStore`]: job runs in PostgreSQL or Oracle, so they survive restarts and servers share
//! them, and [`create_job_tables`] for the schema.
//!
//! Claiming locks due rows with `FOR UPDATE SKIP LOCKED`, so workers never wait on each other and
//! never take the same run. PostgreSQL claims in one `UPDATE ... WHERE id IN (SELECT ... FOR
//! UPDATE SKIP LOCKED) RETURNING`. Oracle (11g and later) selects the ids with `FOR UPDATE SKIP
//! LOCKED` in a transaction, updates them and reads them back before the commit. Times are Unix
//! milliseconds in whole number columns, so both dialects use the same SQL. SQL text is fixed in
//! this file and the only identifier is the checked prefix; every value is a bound parameter.

use super::store::{ClaimedJob, JobStore, NewJob, NextRun, check_new, schedule_id};
use crate::action::{Error, Result};
use crate::ctx::BoxFuture;
use crate::sql::{Arg, Cell, Col, Db, Dialect, Sql, bind, checked_prefix};
use std::sync::Arc;

/// The `CREATE TABLE` and `CREATE INDEX` statements of the job table for a dialect (`postgres` or
/// `oracle`) and table prefix. Put them in a migration.
///
/// ```
/// let sql = zeyn::create_job_tables("postgres", "zeyn_").unwrap();
/// assert!(sql[0].starts_with("CREATE TABLE zeyn_jobs ("));
/// assert!(zeyn::create_job_tables("oracle", "zeyn_").unwrap()[0].contains("NUMBER(19)"));
/// assert!(zeyn::create_job_tables("postgres", "x; DROP TABLE y").is_err());
/// ```
pub fn create_job_tables(dialect: &str, prefix: &str) -> Result<Vec<String>> {
    let p = checked_prefix(prefix)?;
    let (text, big) = types(dialect)?;
    let columns = format!(
        "id {text}(80) PRIMARY KEY, name {text}(64) NOT NULL, payload {text}(4000), state {text}(16) NOT NULL, run_at {big} NOT NULL, attempts {big} NOT NULL, max_attempts {big} NOT NULL, backoff_ms {big} NOT NULL, every_ms {big}, lease_owner {text}(64), lease_until {big}, last_error {text}(64)"
    );
    #[cfg(feature = "metronome")]
    let columns = format!("{columns}, {}", metronome_columns(dialect).join(", "));
    #[cfg_attr(not(feature = "metronome"), allow(unused_mut))]
    let mut all = vec![
        format!("CREATE TABLE {p}jobs ({columns})"),
        format!("CREATE INDEX {p}jobs_due ON {p}jobs (state, run_at)"),
    ];
    #[cfg(feature = "metronome")]
    all.extend(metronome_tables(dialect, &p)?);
    Ok(all)
}

fn types(dialect: &str) -> Result<(&'static str, &'static str)> {
    match dialect {
        "postgres" => Ok(("VARCHAR", "BIGINT")),
        "oracle" => Ok(("VARCHAR2", "NUMBER(19)")),
        other => Err(Error::Internal(format!(
            "no job tables for the `{other}` dialect"
        ))),
    }
}

/// Columns schema version 2 adds to the job table (feature `metronome`).
#[cfg(feature = "metronome")]
fn metronome_columns(dialect: &str) -> Vec<String> {
    let (text, big, small) = match dialect {
        "oracle" => ("VARCHAR2", "NUMBER(19)", "NUMBER(5)"),
        _ => ("VARCHAR", "BIGINT", "SMALLINT"),
    };
    vec![
        format!("trigger_id {text}(64)"),
        format!("tick_key {text}(160)"),
        format!("scheduled_ms {big}"),
        format!("priority {small} DEFAULT 0 NOT NULL"),
        format!("timeout_ms {big}"),
        format!("started_ms {big}"),
        format!("started_by {text}(128)"),
    ]
}

/// Tables and indexes of schema version 2 next to the job table (feature `metronome`).
#[cfg(feature = "metronome")]
fn metronome_tables(dialect: &str, p: &str) -> Result<Vec<String>> {
    let (text, big) = types(dialect)?;
    let (small, int) = match dialect {
        "oracle" => ("NUMBER(5)", "NUMBER(10)"),
        _ => ("SMALLINT", "INTEGER"),
    };
    Ok(vec![
        format!("CREATE UNIQUE INDEX {p}jobs_tick ON {p}jobs (tick_key)"),
        format!(
            "CREATE TABLE {p}job_triggers (id {text}(64) NOT NULL, job {text}(64) NOT NULL, expr {text}(200) NOT NULL, time_zone {text}(64) NOT NULL, calendar {text}(64), misfire {text}(16) NOT NULL, misfire_max {int}, priority {small} DEFAULT 0 NOT NULL, paused {small} DEFAULT 0 NOT NULL, next_ms {big} NOT NULL, updated_ms {big} NOT NULL, PRIMARY KEY (job, id))"
        ),
        format!(
            "CREATE TABLE {p}job_pauses (job {text}(64) PRIMARY KEY, paused_ms {big} NOT NULL)"
        ),
        format!(
            "CREATE TABLE {p}job_runs (id {text}(80) PRIMARY KEY, run_id {text}(80), job {text}(64) NOT NULL, trigger_id {text}(64), scheduled_ms {big}, started_ms {big} NOT NULL, ended_ms {big} NOT NULL, attempt {int} NOT NULL, outcome {text}(16) NOT NULL, error_code {text}(64), worker {text}(64) NOT NULL, started_by {text}(128))"
        ),
        format!("CREATE INDEX {p}job_runs_job ON {p}job_runs (job, ended_ms)"),
        format!("CREATE INDEX {p}job_runs_ended ON {p}job_runs (ended_ms)"),
        format!("CREATE TABLE {p}job_schema (version {int} NOT NULL)"),
        format!("INSERT INTO {p}job_schema (version) VALUES (2)"),
    ])
}

/// The statements that move a job table made by `create_job_tables` without the feature
/// `metronome` (schema version 1) to version 2: new columns, all nullable or with a default so
/// no table is rewritten, and the trigger, pause, history and schema tables. Review them and put
/// them in a migration.
///
/// ```
/// let sql = zeyn::migrate_job_tables("postgres", "zeyn_").unwrap();
/// assert_eq!(sql[0], "ALTER TABLE zeyn_jobs ADD COLUMN trigger_id VARCHAR(64)");
/// assert!(zeyn::migrate_job_tables("oracle", "zeyn_").unwrap()[0].starts_with("ALTER TABLE zeyn_jobs ADD (trigger_id VARCHAR2(64))"));
/// ```
#[cfg(feature = "metronome")]
pub fn migrate_job_tables(dialect: &str, prefix: &str) -> Result<Vec<String>> {
    let p = checked_prefix(prefix)?;
    let mut all: Vec<String> = metronome_columns(dialect)
        .into_iter()
        .map(|c| match dialect {
            "oracle" => format!("ALTER TABLE {p}jobs ADD ({c})"),
            _ => format!("ALTER TABLE {p}jobs ADD COLUMN {c}"),
        })
        .collect();
    all.extend(metronome_tables(dialect, &p)?);
    Ok(all)
}

/// Job runs in a SQL database. Create the table with [`create_job_tables`] (or
/// [`Self::create_tables`] in development) and pass the store to `App::job_store`.
///
/// ```no_run
/// # async fn run(db: zeyn::sql::Db) -> zeyn::Result<()> {
/// let store = zeyn::SqlJobStore::new(db).prefix("shop_")?;
/// store.create_tables().await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct SqlJobStore {
    db: Db,
    prefix: Arc<str>,
}

#[cfg(not(feature = "metronome"))]
const CLAIMED: &str = "id, name, payload, attempts, max_attempts, backoff_ms, every_ms";
#[cfg(feature = "metronome")]
const CLAIMED: &str = "id, name, payload, attempts, max_attempts, backoff_ms, every_ms, trigger_id, scheduled_ms, timeout_ms, started_by, started_ms";
const CLAIMED_COLS: &[Col] = &[
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Int,
    Col::Int,
    Col::Int,
    Col::Int,
    #[cfg(feature = "metronome")]
    Col::Text,
    #[cfg(feature = "metronome")]
    Col::Int,
    #[cfg(feature = "metronome")]
    Col::Int,
    #[cfg(feature = "metronome")]
    Col::Text,
    #[cfg(feature = "metronome")]
    Col::Int,
];
/// What a claim sets besides the lease: the start of the try (feature `metronome`).
#[cfg(not(feature = "metronome"))]
const STARTED: &str = "";
#[cfg(feature = "metronome")]
const STARTED: &str = ", started_ms = ?";
/// Claim order: by priority with aging (feature `metronome`), then oldest first.
#[cfg(not(feature = "metronome"))]
const ORDER: &str = "run_at";
#[cfg(feature = "metronome")]
const ORDER: &str = "CASE WHEN run_at < ? THEN 32767 ELSE priority END DESC, run_at";
#[cfg(feature = "metronome")]
const RANK: &str = "CASE WHEN run_at < ? THEN 32767 ELSE priority END";
const DUE: &str = "((state = 'queued' AND run_at <= ?) OR (state = 'running' AND lease_until < ?))";
const HELD: &str = "id = ? AND lease_owner = ? AND state = 'running'";

fn list(n: usize) -> String {
    let mut out = String::with_capacity(n * 3);
    for i in 0..n {
        out.push_str(if i == 0 { "?" } else { ", ?" });
    }
    out
}

fn claimed_of(row: Vec<Cell>) -> Option<ClaimedJob> {
    let mut c = row.into_iter();
    let text = |c: Option<Cell>| match c {
        Some(Cell::Text(s)) => Some(s),
        _ => None,
    };
    let int = |c: Option<Cell>| match c {
        Some(Cell::Int(n)) => Some(n),
        _ => None,
    };
    #[cfg_attr(not(feature = "metronome"), allow(unused_mut))]
    let mut job = ClaimedJob {
        id: text(c.next())?,
        name: text(c.next())?,
        payload: text(c.next()).unwrap_or_default(),
        attempts: u32::try_from(int(c.next())?).unwrap_or(u32::MAX),
        max_attempts: u32::try_from(int(c.next())?).unwrap_or(1),
        backoff_ms: int(c.next())?,
        every_ms: int(c.next()),
        ..ClaimedJob::default()
    };
    #[cfg(feature = "metronome")]
    {
        job.trigger_id = text(c.next());
        job.scheduled_ms = int(c.next());
        job.timeout_ms = int(c.next());
        job.started_by = text(c.next());
        job.started_ms = int(c.next()).unwrap_or(0);
    }
    Some(job)
}

/// The aging cutoff of a claim: runs due before it count as the highest priority.
#[cfg(feature = "metronome")]
fn aged(now_ms: i64) -> Arg {
    Arg::Int(now_ms.saturating_sub(super::ms(crate::metronome::PRIORITY_AGING)))
}

/// SQL of the crate's own job table for a dialect and checked prefix.
fn sql(d: &dyn Dialect, p: &str, template: &str, args: Vec<Arg>, cols: &[Col]) -> Sql {
    bind(d, p, template, args, cols)
}

/// The claim statement. PostgreSQL: one `UPDATE ... RETURNING`. Oracle: the locking select of
/// the due ids, at most `limit`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_claim(
    d: &dyn Dialect,
    p: &str,
    worker: &str,
    names: &[&'static str],
    now_ms: i64,
    lease_ms: i64,
    limit: usize,
    band: Option<i64>,
) -> Sql {
    let in_names = list(names.len());
    let name_args = names
        .iter()
        .map(|n| Arg::Text((*n).to_string()));
    if d.name() == "oracle" {
        let mut args: Vec<Arg> = name_args.collect();
        args.extend([Arg::Int(now_ms), Arg::Int(now_ms)]);
        #[cfg_attr(not(feature = "metronome"), allow(unused_mut))]
        let mut filter = String::new();
        #[cfg(feature = "metronome")]
        if let Some(band) = band {
            filter = format!(" AND {RANK} >= ?");
            args.extend([aged(now_ms), Arg::Int(band)]);
        }
        #[cfg(not(feature = "metronome"))]
        let _ = band;
        args.push(Arg::Int(limit as i64));
        return sql(
            d,
            p,
            &format!(
                "SELECT id FROM {{p}}jobs WHERE name IN ({in_names}) AND {DUE}{filter} AND ROWNUM <= ? FOR UPDATE SKIP LOCKED"
            ),
            args,
            &[Col::Text],
        );
    }
    let mut args = vec![
        Arg::Text(worker.to_string()),
        Arg::Int(now_ms.saturating_add(lease_ms)),
    ];
    #[cfg(feature = "metronome")]
    args.push(Arg::Int(now_ms));
    args.extend(name_args);
    args.extend([Arg::Int(now_ms), Arg::Int(now_ms)]);
    #[cfg(feature = "metronome")]
    args.push(aged(now_ms));
    args.push(Arg::Int(limit as i64));
    sql(
        d,
        p,
        &format!(
            "UPDATE {{p}}jobs SET state = 'running', lease_owner = ?, lease_until = ?, attempts = attempts + 1{STARTED} WHERE id IN (SELECT id FROM {{p}}jobs WHERE name IN ({in_names}) AND {DUE} ORDER BY {ORDER} LIMIT ? FOR UPDATE SKIP LOCKED) RETURNING {CLAIMED}"
        ),
        args,
        CLAIMED_COLS,
    )
}

/// The highest claim rank among due runs, read without a lock, so an Oracle claim takes that
/// priority band first (feature `metronome`).
#[cfg(feature = "metronome")]
pub(crate) fn plan_band(d: &dyn Dialect, p: &str, names: &[&'static str], now_ms: i64) -> Sql {
    let mut args = vec![aged(now_ms)];
    args.extend(
        names
            .iter()
            .map(|n| Arg::Text((*n).to_string())),
    );
    args.extend([Arg::Int(now_ms), Arg::Int(now_ms)]);
    sql(
        d,
        p,
        &format!(
            "SELECT MAX({RANK}) FROM {{p}}jobs WHERE name IN ({}) AND {DUE}",
            list(names.len())
        ),
        args,
        &[Col::Int],
    )
}

/// The statement that writes the outcome of a try, only while `worker` holds the run.
pub(crate) fn plan_finish(
    d: &dyn Dialect,
    p: &str,
    job: &ClaimedJob,
    worker: &str,
    next: NextRun,
    error: Option<&'static str>,
) -> Sql {
    let held = [Arg::Text(job.id.clone()), Arg::Text(worker.to_string())];
    let error = error.map_or(Arg::Null(Col::Text), |e| Arg::Text(e.to_string()));
    match next {
        NextRun::Done => sql(
            d,
            p,
            &format!("DELETE FROM {{p}}jobs WHERE {HELD}"),
            held.to_vec(),
            &[],
        ),
        NextRun::Queue {
            run_at_ms,
            attempts,
        } => {
            let mut args = vec![Arg::Int(run_at_ms), Arg::Int(i64::from(attempts)), error];
            args.extend(held);
            sql(
                d,
                p,
                &format!(
                    "UPDATE {{p}}jobs SET state = 'queued', run_at = ?, attempts = ?, lease_owner = NULL, lease_until = NULL, last_error = ? WHERE {HELD}"
                ),
                args,
                &[],
            )
        }
        NextRun::Dead => {
            let mut args = vec![error];
            args.extend(held);
            sql(
                d,
                p,
                &format!(
                    "UPDATE {{p}}jobs SET state = 'failed', lease_owner = NULL, lease_until = NULL, last_error = ? WHERE {HELD}"
                ),
                args,
                &[],
            )
        }
    }
}

impl SqlJobStore {
    /// The store over `db`, with the table `zeyn_jobs`.
    pub fn new(db: Db) -> Self {
        SqlJobStore {
            db,
            prefix: Arc::from("zeyn_"),
        }
    }

    /// Another table prefix: lower case letters, digits and `_`, at most 16 characters.
    pub fn prefix(mut self, prefix: &str) -> Result<Self> {
        self.prefix = Arc::from(checked_prefix(prefix)?);
        Ok(self)
    }

    /// Creates the table. For development and tests; production schemas belong in migrations
    /// made from [`create_job_tables`].
    pub async fn create_tables(&self) -> Result<()> {
        for statement in create_job_tables(self.db.dialect().name(), &self.prefix)? {
            self.db
                .run(Sql {
                    text: statement,
                    args: Vec::new(),
                    cols: Vec::new(),
                })
                .await?;
        }
        Ok(())
    }

    fn sql(&self, template: &str, args: Vec<Arg>, cols: &[Col]) -> Sql {
        sql(self.db.dialect(), &self.prefix, template, args, cols)
    }

    async fn claim_oracle(
        &self,
        worker: String,
        names: Vec<&'static str>,
        now_ms: i64,
        lease_ms: i64,
        limit: usize,
    ) -> Result<Vec<ClaimedJob>> {
        let mut tx = self.db.begin().await?;
        #[cfg(not(feature = "metronome"))]
        let band = None;
        #[cfg(feature = "metronome")]
        let band = {
            tx.rows(plan_band(self.db.dialect(), &self.prefix, &names, now_ms))
                .await?
                .into_iter()
                .next()
                .and_then(|r| match r.into_iter().next() {
                    Some(Cell::Int(n)) => Some(n),
                    _ => None,
                })
        };
        let first = plan_claim(
            self.db.dialect(),
            &self.prefix,
            &worker,
            &names,
            now_ms,
            lease_ms,
            limit,
            band,
        );
        let ids: Vec<Arg> = tx
            .rows(first)
            .await?
            .into_iter()
            .filter_map(|r| match r.into_iter().next() {
                Some(Cell::Text(id)) => Some(Arg::Text(id)),
                _ => None,
            })
            .collect();
        if ids.is_empty() {
            tx.rollback().await?;
            return Ok(Vec::new());
        }
        let in_ids = list(ids.len());
        let until = now_ms.saturating_add(lease_ms);
        let mut args = vec![Arg::Text(worker.clone()), Arg::Int(until)];
        #[cfg(feature = "metronome")]
        args.push(Arg::Int(now_ms));
        args.extend(ids.iter().cloned());
        tx.run(self.sql(
            &format!(
                "UPDATE {{p}}jobs SET state = 'running', lease_owner = ?, lease_until = ?, attempts = attempts + 1{STARTED} WHERE id IN ({in_ids})"
            ),
            args,
            &[],
        ))
        .await?;
        let mut args = ids;
        args.extend([Arg::Text(worker), Arg::Int(until)]);
        let rows = tx
            .rows(self.sql(
                &format!(
                    "SELECT {CLAIMED} FROM {{p}}jobs WHERE id IN ({in_ids}) AND lease_owner = ? AND lease_until = ? ORDER BY run_at"
                ),
                args,
                CLAIMED_COLS,
            ))
            .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .filter_map(claimed_of)
            .collect())
    }

    /// The insert of a queued run.
    fn insert(&self, id: &str, job: NewJob) -> Sql {
        #[cfg(not(feature = "metronome"))]
        return self.sql(
            "INSERT INTO {p}jobs (id, name, payload, state, run_at, attempts, max_attempts, backoff_ms) VALUES (?, ?, ?, 'queued', ?, 0, ?, ?)",
            vec![
                Arg::Text(id.to_string()),
                Arg::Text(job.name),
                Arg::Text(job.payload),
                Arg::Int(job.run_at_ms),
                Arg::Int(i64::from(job.max_attempts.max(1))),
                Arg::Int(job.backoff_ms),
            ],
            &[],
        );
        #[cfg(feature = "metronome")]
        self.sql(
            "INSERT INTO {p}jobs (id, name, payload, state, run_at, attempts, max_attempts, backoff_ms, trigger_id, tick_key, scheduled_ms, priority, timeout_ms, started_by) VALUES (?, ?, ?, 'queued', ?, 0, ?, ?, ?, ?, ?, ?, ?, ?)",
            vec![
                Arg::Text(id.to_string()),
                Arg::Text(job.name),
                Arg::Text(job.payload),
                Arg::Int(job.run_at_ms),
                Arg::Int(i64::from(job.max_attempts.max(1))),
                Arg::Int(job.backoff_ms),
                text_or_null(job.trigger_id),
                text_or_null(job.tick_key),
                int_or_null(job.scheduled_ms),
                Arg::Int(i64::from(job.priority)),
                int_or_null(job.timeout_ms),
                text_or_null(job.started_by),
            ],
            &[],
        )
    }

    /// Writes the outcome of a try and, with the feature `metronome`, its history row in the
    /// same transaction.
    async fn finish(
        &self,
        job: &ClaimedJob,
        worker: &str,
        next: NextRun,
        error: Option<&'static str>,
        now_ms: i64,
    ) -> Result<bool> {
        let finish = plan_finish(self.db.dialect(), &self.prefix, job, worker, next, error);
        #[cfg(not(feature = "metronome"))]
        {
            let _ = now_ms;
            Ok(self.db.run(finish).await? > 0)
        }
        #[cfg(feature = "metronome")]
        {
            let mut tx = self.db.begin().await?;
            if tx.run(finish).await? == 0 {
                tx.rollback().await?;
                return Ok(false);
            }
            tx.run(self.history_insert(&super::store::run_row(job, worker, error, now_ms)))
                .await?;
            tx.commit().await?;
            Ok(true)
        }
    }
}

#[cfg(feature = "metronome")]
fn text_or_null(v: Option<String>) -> Arg {
    v.map_or(Arg::Null(Col::Text), Arg::Text)
}

#[cfg(feature = "metronome")]
fn int_or_null(v: Option<i64>) -> Arg {
    v.map_or(Arg::Null(Col::Int), Arg::Int)
}

#[cfg(feature = "metronome")]
const TRIGGER: &str =
    "job, id, expr, time_zone, calendar, misfire, misfire_max, priority, paused, next_ms";
#[cfg(feature = "metronome")]
const TRIGGER_COLS: &[Col] = &[
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Int,
    Col::Int,
    Col::Int,
    Col::Int,
];
#[cfg(feature = "metronome")]
const RUN: &str = "id, run_id, job, trigger_id, scheduled_ms, started_ms, ended_ms, attempt, outcome, error_code, worker, started_by";
#[cfg(feature = "metronome")]
const RUN_COLS: &[Col] = &[
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Int,
    Col::Int,
    Col::Int,
    Col::Int,
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Text,
];

#[cfg(feature = "metronome")]
fn cell_text(c: Option<Cell>) -> Option<String> {
    match c {
        Some(Cell::Text(s)) => Some(s),
        _ => None,
    }
}

#[cfg(feature = "metronome")]
fn cell_int(c: Option<Cell>) -> Option<i64> {
    match c {
        Some(Cell::Int(n)) => Some(n),
        _ => None,
    }
}

#[cfg(feature = "metronome")]
fn trigger_of(row: Vec<Cell>) -> Option<crate::metronome::TriggerRow> {
    let mut c = row.into_iter();
    Some(crate::metronome::TriggerRow {
        job: cell_text(c.next())?,
        id: cell_text(c.next())?,
        expr: cell_text(c.next())?,
        time_zone: cell_text(c.next())?,
        calendar: cell_text(c.next()),
        misfire: cell_text(c.next())?,
        misfire_max: cell_int(c.next()).and_then(|n| u32::try_from(n).ok()),
        priority: i16::try_from(cell_int(c.next())?).unwrap_or(0),
        paused: cell_int(c.next())? != 0,
        next_ms: cell_int(c.next())?,
    })
}

#[cfg(feature = "metronome")]
fn run_of(row: Vec<Cell>) -> Option<crate::metronome::RunRow> {
    let mut c = row.into_iter();
    Some(crate::metronome::RunRow {
        id: cell_text(c.next())?,
        run_id: cell_text(c.next()).unwrap_or_default(),
        job: cell_text(c.next())?,
        trigger_id: cell_text(c.next()),
        scheduled_ms: cell_int(c.next()),
        started_ms: cell_int(c.next())?,
        ended_ms: cell_int(c.next())?,
        attempt: u32::try_from(cell_int(c.next())?).unwrap_or(0),
        outcome: cell_text(c.next())?,
        error_code: cell_text(c.next()),
        worker: cell_text(c.next())?,
        started_by: cell_text(c.next()),
    })
}

#[cfg(feature = "metronome")]
impl SqlJobStore {
    fn history_insert(&self, r: &crate::metronome::RunRow) -> Sql {
        self.sql(
            &format!(
                "INSERT INTO {{p}}job_runs ({RUN}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            ),
            vec![
                Arg::Text(r.id.clone()),
                text_or_null(Some(r.run_id.clone()).filter(|s| !s.is_empty())),
                Arg::Text(r.job.clone()),
                text_or_null(r.trigger_id.clone()),
                int_or_null(r.scheduled_ms),
                Arg::Int(r.started_ms),
                Arg::Int(r.ended_ms),
                Arg::Int(i64::from(r.attempt)),
                Arg::Text(r.outcome.clone()),
                text_or_null(r.error_code.clone()),
                Arg::Text(r.worker.clone()),
                text_or_null(r.started_by.clone()),
            ],
            &[],
        )
    }

    async fn trigger_row(
        &self,
        job: &str,
        id: &str,
    ) -> Result<Option<crate::metronome::TriggerRow>> {
        Ok(self
            .db
            .rows(self.sql(
                &format!("SELECT {TRIGGER} FROM {{p}}job_triggers WHERE job = ? AND id = ?"),
                vec![Arg::Text(job.into()), Arg::Text(id.into())],
                TRIGGER_COLS,
            ))
            .await?
            .into_iter()
            .next()
            .and_then(trigger_of))
    }

    async fn add_trigger_row(
        &self,
        row: crate::metronome::TriggerRow,
    ) -> Result<crate::metronome::TriggerRow> {
        let now = crate::time::DateTime::now().unix_ms();
        let max = row.misfire_max.map(i64::from);
        let Some(stored) = self
            .trigger_row(&row.job, &row.id)
            .await?
        else {
            let inserted = self
                .db
                .run(self.sql(
                    &format!("INSERT INTO {{p}}job_triggers ({TRIGGER}, updated_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)"),
                    vec![
                        Arg::Text(row.job.clone()),
                        Arg::Text(row.id.clone()),
                        Arg::Text(row.expr.clone()),
                        Arg::Text(row.time_zone.clone()),
                        text_or_null(row.calendar.clone()),
                        Arg::Text(row.misfire.clone()),
                        int_or_null(max),
                        Arg::Int(i64::from(row.priority)),
                        Arg::Int(row.next_ms),
                        Arg::Int(now),
                    ],
                    &[],
                ))
                .await;
            return match inserted {
                Ok(_) => Ok(row),
                Err(Error::Validation(_)) => self
                    .trigger_row(&row.job, &row.id)
                    .await?
                    .ok_or(Error::NotFound),
                Err(e) => Err(e),
            };
        };
        let moved = stored.expr != row.expr || stored.time_zone != row.time_zone;
        let next = if moved { row.next_ms } else { stored.next_ms };
        self.db
            .run(self.sql(
                "UPDATE {p}job_triggers SET expr = ?, time_zone = ?, calendar = ?, misfire = ?, misfire_max = ?, priority = ?, next_ms = ?, updated_ms = ? WHERE job = ? AND id = ?",
                vec![
                    Arg::Text(row.expr.clone()),
                    Arg::Text(row.time_zone.clone()),
                    text_or_null(row.calendar.clone()),
                    Arg::Text(row.misfire.clone()),
                    int_or_null(max),
                    Arg::Int(i64::from(row.priority)),
                    Arg::Int(next),
                    Arg::Int(now),
                    Arg::Text(row.job.clone()),
                    Arg::Text(row.id.clone()),
                ],
                &[],
            ))
            .await?;
        Ok(crate::metronome::TriggerRow {
            paused: stored.paused,
            next_ms: next,
            ..row
        })
    }

    async fn fire_tx(
        &self,
        job: String,
        trigger: String,
        from_ms: i64,
        to_ms: i64,
        runs: Vec<NewJob>,
        skipped: Option<crate::metronome::RunRow>,
    ) -> Result<Option<i64>> {
        let mut tx = self.db.begin().await?;
        let moved = tx
            .run(self.sql(
                "UPDATE {p}job_triggers SET next_ms = ?, updated_ms = ? WHERE job = ? AND id = ? AND next_ms = ?",
                vec![
                    Arg::Int(to_ms),
                    Arg::Int(crate::time::DateTime::now().unix_ms()),
                    Arg::Text(job.clone()),
                    Arg::Text(trigger.clone()),
                    Arg::Int(from_ms),
                ],
                &[],
            ))
            .await?;
        if moved == 0 {
            let stored = tx
                .rows(self.sql(
                    "SELECT next_ms FROM {p}job_triggers WHERE job = ? AND id = ?",
                    vec![Arg::Text(job), Arg::Text(trigger)],
                    &[Col::Int],
                ))
                .await?
                .into_iter()
                .next()
                .and_then(|r| cell_int(r.into_iter().next()));
            tx.rollback().await?;
            return stored.map(Some).ok_or(Error::NotFound);
        }
        for run in runs {
            check_new(&run)?;
            tx.run(self.insert(&super::random_id(), run))
                .await?;
        }
        if let Some(row) = skipped {
            tx.run(self.history_insert(&row))
                .await?;
        }
        tx.commit().await?;
        Ok(None)
    }

    async fn prune(&self, before_ms: i64, keep: u64, limit: usize) -> Result<u64> {
        let oracle = self.db.dialect().name() == "oracle";
        let keep = i64::try_from(keep).unwrap_or(i64::MAX);
        let edge = if oracle {
            self.sql(
                "SELECT ended_ms FROM (SELECT ended_ms, ROWNUM rn FROM (SELECT ended_ms FROM {p}job_runs ORDER BY ended_ms DESC) WHERE ROWNUM <= ?) WHERE rn = ?",
                vec![Arg::Int(keep.saturating_add(1)), Arg::Int(keep.saturating_add(1))],
                &[Col::Int],
            )
        } else {
            self.sql(
                "SELECT ended_ms FROM {p}job_runs ORDER BY ended_ms DESC LIMIT 1 OFFSET ?",
                vec![Arg::Int(keep)],
                &[Col::Int],
            )
        };
        let edge = self
            .db
            .rows(edge)
            .await?
            .into_iter()
            .next()
            .and_then(|r| cell_int(r.into_iter().next()));
        let cutoff = edge.map_or(before_ms, |e| before_ms.max(e.saturating_add(1)));
        let delete = if oracle {
            "DELETE FROM {p}job_runs WHERE ended_ms < ? AND ROWNUM <= ?"
        } else {
            "DELETE FROM {p}job_runs WHERE id IN (SELECT id FROM {p}job_runs WHERE ended_ms < ? LIMIT ?)"
        };
        self.db
            .run(self.sql(delete, vec![Arg::Int(cutoff), Arg::Int(limit as i64)], &[]))
            .await
    }
}

impl JobStore for SqlJobStore {
    fn enqueue(&self, job: NewJob) -> BoxFuture<Result<String>> {
        let s = self.clone();
        Box::pin(async move {
            check_new(&job)?;
            let id = super::random_id();
            s.db.run(s.insert(&id, job)).await?;
            Ok(id)
        })
    }

    fn schedule(
        &self,
        name: &str,
        every_ms: i64,
        first_run_ms: i64,
        max_attempts: u32,
        backoff_ms: i64,
    ) -> BoxFuture<Result<()>> {
        let (s, name) = (self.clone(), name.to_string());
        Box::pin(async move {
            let id = schedule_id(&name);
            let max = i64::from(max_attempts.max(1));
            let changed =
                s.db.run(s.sql(
                    "UPDATE {p}jobs SET every_ms = ?, max_attempts = ?, backoff_ms = ? WHERE id = ?",
                    vec![Arg::Int(every_ms), Arg::Int(max), Arg::Int(backoff_ms), Arg::Text(id.clone())],
                    &[],
                ))
                .await?;
            if changed > 0 {
                return Ok(());
            }
            let inserted = s
                .db
                .run(s.sql(
                    "INSERT INTO {p}jobs (id, name, state, run_at, attempts, max_attempts, backoff_ms, every_ms) VALUES (?, ?, 'queued', ?, 0, ?, ?, ?)",
                    vec![
                        Arg::Text(id),
                        Arg::Text(name),
                        Arg::Int(first_run_ms),
                        Arg::Int(max),
                        Arg::Int(backoff_ms),
                        Arg::Int(every_ms),
                    ],
                    &[],
                ))
                .await;
            match inserted {
                Ok(_) | Err(Error::Validation(_)) => Ok(()),
                Err(e) => Err(e),
            }
        })
    }

    fn claim(
        &self,
        worker: &str,
        names: &[&'static str],
        now_ms: i64,
        lease_ms: i64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<ClaimedJob>>> {
        let s = self.clone();
        let worker = worker.to_string();
        let names = names.to_vec();
        Box::pin(async move {
            if names.is_empty() || limit == 0 {
                return Ok(Vec::new());
            }
            if s.db.dialect().name() == "oracle" {
                return s
                    .claim_oracle(worker, names, now_ms, lease_ms, limit)
                    .await;
            }
            let plan = plan_claim(
                s.db.dialect(),
                &s.prefix,
                &worker,
                &names,
                now_ms,
                lease_ms,
                limit,
                None,
            );
            Ok(s.db
                .rows(plan)
                .await?
                .into_iter()
                .filter_map(claimed_of)
                .collect())
        })
    }

    fn heartbeat(&self, id: &str, worker: &str, lease_until_ms: i64) -> BoxFuture<Result<bool>> {
        let s = self.clone();
        let sql = s.sql(
            &format!("UPDATE {{p}}jobs SET lease_until = ? WHERE {HELD}"),
            vec![
                Arg::Int(lease_until_ms),
                Arg::Text(id.to_string()),
                Arg::Text(worker.to_string()),
            ],
            &[],
        );
        Box::pin(async move { Ok(s.db.run(sql).await? > 0) })
    }

    fn complete(&self, job: &ClaimedJob, worker: &str, now_ms: i64) -> BoxFuture<Result<bool>> {
        let (s, job, worker) = (self.clone(), job.clone(), worker.to_string());
        Box::pin(async move {
            s.finish(&job, &worker, job.after_success(now_ms), None, now_ms)
                .await
        })
    }

    fn fail(
        &self,
        job: &ClaimedJob,
        worker: &str,
        error: &'static str,
        now_ms: i64,
    ) -> BoxFuture<Result<bool>> {
        let (s, job, worker) = (self.clone(), job.clone(), worker.to_string());
        Box::pin(async move {
            s.finish(
                &job,
                &worker,
                job.after_failure(now_ms),
                Some(error),
                now_ms,
            )
            .await
        })
    }

    #[cfg(feature = "metronome")]
    fn add_trigger(
        &self,
        row: crate::metronome::TriggerRow,
    ) -> BoxFuture<Result<crate::metronome::TriggerRow>> {
        let s = self.clone();
        Box::pin(async move { s.add_trigger_row(row).await })
    }

    #[cfg(feature = "metronome")]
    fn triggers(&self) -> BoxFuture<Result<Vec<crate::metronome::TriggerRow>>> {
        let s = self.clone();
        Box::pin(async move {
            Ok(s.db
                .rows(s.sql(
                    &format!("SELECT {TRIGGER} FROM {{p}}job_triggers"),
                    Vec::new(),
                    TRIGGER_COLS,
                ))
                .await?
                .into_iter()
                .filter_map(trigger_of)
                .collect())
        })
    }

    #[cfg(feature = "metronome")]
    fn fire(
        &self,
        job: &str,
        trigger: &str,
        from_ms: i64,
        to_ms: i64,
        runs: Vec<NewJob>,
        skipped: Option<crate::metronome::RunRow>,
    ) -> BoxFuture<Result<Option<i64>>> {
        let (s, job, trigger) = (self.clone(), job.to_string(), trigger.to_string());
        Box::pin(async move {
            s.fire_tx(job, trigger, from_ms, to_ms, runs, skipped)
                .await
        })
    }

    #[cfg(feature = "metronome")]
    fn move_trigger(&self, job: &str, trigger: &str, next_ms: i64) -> BoxFuture<Result<()>> {
        let s = self.clone();
        let sql = s.sql(
            "UPDATE {p}job_triggers SET next_ms = ? WHERE job = ? AND id = ?",
            vec![
                Arg::Int(next_ms),
                Arg::Text(job.into()),
                Arg::Text(trigger.into()),
            ],
            &[],
        );
        Box::pin(async move { s.db.run(sql).await.map(|_| ()) })
    }

    #[cfg(feature = "metronome")]
    fn set_paused(
        &self,
        job: &str,
        trigger: Option<&str>,
        paused: bool,
        now_ms: i64,
    ) -> BoxFuture<Result<()>> {
        let s = self.clone();
        let (job, trigger) = (job.to_string(), trigger.map(str::to_string));
        Box::pin(async move {
            if let Some(id) = trigger {
                s.db.run(s.sql(
                    "UPDATE {p}job_triggers SET paused = ? WHERE job = ? AND id = ?",
                    vec![Arg::Int(i64::from(paused)), Arg::Text(job), Arg::Text(id)],
                    &[],
                ))
                .await?;
                return Ok(());
            }
            s.db.run(s.sql(
                "DELETE FROM {p}job_pauses WHERE job = ?",
                vec![Arg::Text(job.clone())],
                &[],
            ))
            .await?;
            if paused {
                match s
                    .db
                    .run(s.sql(
                        "INSERT INTO {p}job_pauses (job, paused_ms) VALUES (?, ?)",
                        vec![Arg::Text(job), Arg::Int(now_ms)],
                        &[],
                    ))
                    .await
                {
                    Ok(_) | Err(Error::Validation(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        })
    }

    #[cfg(feature = "metronome")]
    fn paused_jobs(&self) -> BoxFuture<Result<Vec<String>>> {
        let s = self.clone();
        Box::pin(async move {
            Ok(s.db
                .rows(s.sql("SELECT job FROM {p}job_pauses", Vec::new(), &[Col::Text]))
                .await?
                .into_iter()
                .filter_map(|r| cell_text(r.into_iter().next()))
                .collect())
        })
    }

    #[cfg(feature = "metronome")]
    fn history(
        &self,
        job: Option<&str>,
        offset: u64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<crate::metronome::RunRow>>> {
        let s = self.clone();
        let mut args: Vec<Arg> = job
            .map(|j| Arg::Text(j.into()))
            .into_iter()
            .collect();
        let filter = if job.is_some() { " WHERE job = ?" } else { "" };
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let limit = limit as i64;
        let text = if s.db.dialect().name() == "oracle" {
            args.extend([Arg::Int(offset.saturating_add(limit)), Arg::Int(offset)]);
            format!(
                "SELECT {RUN} FROM (SELECT a.*, ROWNUM rn FROM (SELECT {RUN} FROM {{p}}job_runs{filter} ORDER BY ended_ms DESC, id) a WHERE ROWNUM <= ?) WHERE rn > ?"
            )
        } else {
            args.extend([Arg::Int(limit), Arg::Int(offset)]);
            format!(
                "SELECT {RUN} FROM {{p}}job_runs{filter} ORDER BY ended_ms DESC, id LIMIT ? OFFSET ?"
            )
        };
        let sql = s.sql(&text, args, RUN_COLS);
        Box::pin(async move {
            Ok(s.db
                .rows(sql)
                .await?
                .into_iter()
                .filter_map(run_of)
                .collect())
        })
    }

    #[cfg(feature = "metronome")]
    fn prune_history(&self, before_ms: i64, keep: u64, limit: usize) -> BoxFuture<Result<u64>> {
        let s = self.clone();
        Box::pin(async move { s.prune(before_ms, keep, limit).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::{Oracle, OracleCompat, Postgres};

    fn job() -> ClaimedJob {
        ClaimedJob {
            id: "r1".into(),
            name: "mail".into(),
            payload: String::new(),
            attempts: 1,
            max_attempts: 3,
            backoff_ms: 100,
            every_ms: None,
            ..ClaimedJob::default()
        }
    }

    #[cfg(not(feature = "metronome"))]
    #[test]
    fn postgres_claims_in_one_statement_with_skip_locked_and_binds_every_value() {
        let sql = plan_claim(
            &Postgres,
            "zeyn_",
            "w1",
            &["mail", "report"],
            1_000,
            60_000,
            4,
            None,
        );
        assert_eq!(
            sql.text,
            "UPDATE zeyn_jobs SET state = 'running', lease_owner = $1, lease_until = $2, attempts = attempts + 1 WHERE id IN (SELECT id FROM zeyn_jobs WHERE name IN ($3, $4) AND ((state = 'queued' AND run_at <= $5) OR (state = 'running' AND lease_until < $6)) ORDER BY run_at LIMIT $7 FOR UPDATE SKIP LOCKED) RETURNING id, name, payload, attempts, max_attempts, backoff_ms, every_ms"
        );
        assert_eq!(
            sql.args,
            vec![
                Arg::Text("w1".into()),
                Arg::Int(61_000),
                Arg::Text("mail".into()),
                Arg::Text("report".into()),
                Arg::Int(1_000),
                Arg::Int(1_000),
                Arg::Int(4)
            ]
        );
        assert_eq!(sql.cols, CLAIMED_COLS.to_vec());
    }

    #[cfg(not(feature = "metronome"))]
    #[test]
    fn oracle_11g_selects_ids_with_rownum_and_skip_locked() {
        let d = Oracle {
            compat: OracleCompat::V11,
        };
        let sql = plan_claim(&d, "zeyn_", "w1", &["mail"], 1_000, 60_000, 2, None);
        assert_eq!(
            sql.text,
            "SELECT id FROM zeyn_jobs WHERE name IN (:1) AND ((state = 'queued' AND run_at <= :2) OR (state = 'running' AND lease_until < :3)) AND ROWNUM <= :4 FOR UPDATE SKIP LOCKED"
        );
        assert!(
            !sql.text.contains("FETCH") && !sql.text.contains("ORDER BY"),
            "11g syntax, no ORDER BY under FOR UPDATE"
        );
        assert_eq!(sql.args.len(), 4);
    }

    #[test]
    fn finishing_only_changes_a_run_the_worker_still_holds() {
        let j = job();
        let done = plan_finish(&Postgres, "p_", &j, "w1", NextRun::Done, None);
        assert_eq!(
            done.text,
            "DELETE FROM p_jobs WHERE id = $1 AND lease_owner = $2 AND state = 'running'"
        );
        let retry = plan_finish(
            &Postgres,
            "p_",
            &j,
            "w1",
            j.after_failure(1_000),
            Some("internal"),
        );
        assert!(
            retry
                .text
                .starts_with("UPDATE p_jobs SET state = 'queued', run_at = $1, attempts = $2")
        );
        assert!(
            retry
                .text
                .ends_with("WHERE id = $4 AND lease_owner = $5 AND state = 'running'")
        );
        assert_eq!(retry.args[0], Arg::Int(1_100));
        assert_eq!(retry.args[2], Arg::Text("internal".into()));
        let dead = plan_finish(
            &Postgres,
            "p_",
            &ClaimedJob { attempts: 3, ..j },
            "w1",
            NextRun::Dead,
            Some("internal"),
        );
        assert!(dead.text.contains("state = 'failed'"));
    }

    #[test]
    fn the_schema_uses_whole_number_times_and_checks_the_prefix() {
        for dialect in ["postgres", "oracle"] {
            let all = create_job_tables(dialect, "shop_")
                .unwrap()
                .join("\n");
            assert!(all.contains("CREATE TABLE shop_jobs ("));
            assert!(all.contains("CREATE INDEX shop_jobs_due ON shop_jobs (state, run_at)"));
            assert!(
                !all.contains("TIMESTAMP") && !all.contains("IDENTITY"),
                "works on Oracle 11g"
            );
        }
        assert!(create_job_tables("oracle", "z_").unwrap()[0].contains("VARCHAR2(4000)"));
        assert!(create_job_tables("postgres", "Bad").is_err());
        assert!(create_job_tables("sqlite", "z_").is_err());
    }

    #[cfg(feature = "metronome")]
    #[test]
    fn claims_order_by_priority_with_aging_and_record_the_start() {
        let sql = plan_claim(
            &Postgres,
            "zeyn_",
            "w1",
            &["mail"],
            400_000,
            60_000,
            4,
            None,
        );
        assert!(
            sql.text
                .contains("attempts = attempts + 1, started_ms = $3 WHERE")
        );
        assert!(sql.text.contains(
            "ORDER BY CASE WHEN run_at < $7 THEN 32767 ELSE priority END DESC, run_at LIMIT $8"
        ));
        assert_eq!(sql.args[2], Arg::Int(400_000));
        assert_eq!(sql.args[6], Arg::Int(100_000), "five minutes of aging");
        let d = Oracle {
            compat: OracleCompat::V11,
        };
        let band = plan_claim(&d, "zeyn_", "w1", &["mail"], 400_000, 60_000, 2, Some(10));
        assert!(band.text.contains(
            "AND CASE WHEN run_at < :4 THEN 32767 ELSE priority END >= :5 AND ROWNUM <= :6 FOR UPDATE SKIP LOCKED"
        ));
        assert!(
            !band.text.contains("ORDER BY"),
            "no ORDER BY under FOR UPDATE on 11g"
        );
        let max = plan_band(&d, "zeyn_", &["mail"], 400_000);
        assert!(
            max.text
                .starts_with("SELECT MAX(CASE WHEN run_at < :1")
        );
    }

    #[cfg(feature = "metronome")]
    #[test]
    fn schema_version_2_adds_triggers_history_and_a_unique_tick_within_oracle_name_limits() {
        for dialect in ["postgres", "oracle"] {
            let all = create_job_tables(dialect, "tenant8_")
                .unwrap()
                .join("\n");
            assert!(
                all.contains("CREATE UNIQUE INDEX tenant8_jobs_tick ON tenant8_jobs (tick_key)")
            );
            for table in ["job_triggers", "job_pauses", "job_runs", "job_schema"] {
                assert!(
                    all.contains(&format!("CREATE TABLE tenant8_{table} (")),
                    "{table}"
                );
            }
            for name in all
                .split_whitespace()
                .filter(|w| w.starts_with("tenant8_"))
            {
                let name = name.trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
                assert!(name.len() <= 30, "{name} is longer than Oracle 11g allows");
            }
            let migrate = migrate_job_tables(dialect, "tenant8_").unwrap();
            assert_eq!(
                migrate
                    .iter()
                    .filter(|s| s.starts_with("ALTER TABLE"))
                    .count(),
                7
            );
            assert!(
                !migrate
                    .iter()
                    .any(|s| s.contains("NOT NULL")
                        && s.starts_with("ALTER")
                        && !s.contains("DEFAULT"))
            );
        }
        assert!(
            create_job_tables("oracle", "z_").unwrap()[0]
                .contains("priority NUMBER(5) DEFAULT 0 NOT NULL")
        );
    }
}
