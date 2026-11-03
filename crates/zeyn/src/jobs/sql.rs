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
    let (text, big) = match dialect {
        "postgres" => ("VARCHAR", "BIGINT"),
        "oracle" => ("VARCHAR2", "NUMBER(19)"),
        other => {
            return Err(Error::Internal(format!(
                "no job tables for the `{other}` dialect"
            )));
        }
    };
    Ok(vec![
        format!(
            "CREATE TABLE {p}jobs (id {text}(80) PRIMARY KEY, name {text}(64) NOT NULL, payload {text}(4000), state {text}(16) NOT NULL, run_at {big} NOT NULL, attempts {big} NOT NULL, max_attempts {big} NOT NULL, backoff_ms {big} NOT NULL, every_ms {big}, lease_owner {text}(64), lease_until {big}, last_error {text}(64))"
        ),
        format!("CREATE INDEX {p}jobs_due ON {p}jobs (state, run_at)"),
    ])
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

const CLAIMED: &str = "id, name, payload, attempts, max_attempts, backoff_ms, every_ms";
const CLAIMED_COLS: [Col; 7] = [
    Col::Text,
    Col::Text,
    Col::Text,
    Col::Int,
    Col::Int,
    Col::Int,
    Col::Int,
];
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
    Some(ClaimedJob {
        id: text(c.next())?,
        name: text(c.next())?,
        payload: text(c.next()).unwrap_or_default(),
        attempts: u32::try_from(int(c.next())?).unwrap_or(u32::MAX),
        max_attempts: u32::try_from(int(c.next())?).unwrap_or(1),
        backoff_ms: int(c.next())?,
        every_ms: int(c.next()),
    })
}

/// SQL of the crate's own job table for a dialect and checked prefix.
fn sql(d: &dyn Dialect, p: &str, template: &str, args: Vec<Arg>, cols: &[Col]) -> Sql {
    bind(d, p, template, args, cols)
}

/// The claim statement. PostgreSQL: one `UPDATE ... RETURNING`. Oracle: the locking select of
/// the due ids, at most `limit`.
pub(crate) fn plan_claim(
    d: &dyn Dialect,
    p: &str,
    worker: &str,
    names: &[&'static str],
    now_ms: i64,
    lease_ms: i64,
    limit: usize,
) -> Sql {
    let in_names = list(names.len());
    let name_args = names
        .iter()
        .map(|n| Arg::Text((*n).to_string()));
    if d.name() == "oracle" {
        let mut args: Vec<Arg> = name_args.collect();
        args.extend([Arg::Int(now_ms), Arg::Int(now_ms), Arg::Int(limit as i64)]);
        return sql(
            d,
            p,
            &format!(
                "SELECT id FROM {{p}}jobs WHERE name IN ({in_names}) AND {DUE} AND ROWNUM <= ? FOR UPDATE SKIP LOCKED"
            ),
            args,
            &[Col::Text],
        );
    }
    let mut args = vec![
        Arg::Text(worker.to_string()),
        Arg::Int(now_ms.saturating_add(lease_ms)),
    ];
    args.extend(name_args);
    args.extend([Arg::Int(now_ms), Arg::Int(now_ms), Arg::Int(limit as i64)]);
    sql(
        d,
        p,
        &format!(
            "UPDATE {{p}}jobs SET state = 'running', lease_owner = ?, lease_until = ?, attempts = attempts + 1 WHERE id IN (SELECT id FROM {{p}}jobs WHERE name IN ({in_names}) AND {DUE} ORDER BY run_at LIMIT ? FOR UPDATE SKIP LOCKED) RETURNING {CLAIMED}"
        ),
        args,
        &CLAIMED_COLS,
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
        first: Sql,
        now_ms: i64,
        lease_ms: i64,
    ) -> Result<Vec<ClaimedJob>> {
        let mut tx = self.db.begin().await?;
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
        args.extend(ids.iter().cloned());
        tx.run(self.sql(
            &format!(
                "UPDATE {{p}}jobs SET state = 'running', lease_owner = ?, lease_until = ?, attempts = attempts + 1 WHERE id IN ({in_ids})"
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
                &CLAIMED_COLS,
            ))
            .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .filter_map(claimed_of)
            .collect())
    }
}

impl JobStore for SqlJobStore {
    fn enqueue(&self, job: NewJob) -> BoxFuture<Result<String>> {
        let s = self.clone();
        Box::pin(async move {
            check_new(&job)?;
            let id = super::random_id();
            s.db.run(s.sql(
                "INSERT INTO {p}jobs (id, name, payload, state, run_at, attempts, max_attempts, backoff_ms) VALUES (?, ?, ?, 'queued', ?, 0, ?, ?)",
                vec![
                    Arg::Text(id.clone()),
                    Arg::Text(job.name),
                    Arg::Text(job.payload),
                    Arg::Int(job.run_at_ms),
                    Arg::Int(i64::from(job.max_attempts.max(1))),
                    Arg::Int(job.backoff_ms),
                ],
                &[],
            ))
            .await?;
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
        let plan = (!names.is_empty() && limit > 0).then(|| {
            plan_claim(
                s.db.dialect(),
                &s.prefix,
                &worker,
                names,
                now_ms,
                lease_ms,
                limit,
            )
        });
        Box::pin(async move {
            let Some(plan) = plan else {
                return Ok(Vec::new());
            };
            if s.db.dialect().name() == "oracle" {
                return s
                    .claim_oracle(worker, plan, now_ms, lease_ms)
                    .await;
            }
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
        let s = self.clone();
        let sql = plan_finish(
            s.db.dialect(),
            &s.prefix,
            job,
            worker,
            job.after_success(now_ms),
            None,
        );
        Box::pin(async move { Ok(s.db.run(sql).await? > 0) })
    }

    fn fail(
        &self,
        job: &ClaimedJob,
        worker: &str,
        error: &'static str,
        now_ms: i64,
    ) -> BoxFuture<Result<bool>> {
        let s = self.clone();
        let sql = plan_finish(
            s.db.dialect(),
            &s.prefix,
            job,
            worker,
            job.after_failure(now_ms),
            Some(error),
        );
        Box::pin(async move { Ok(s.db.run(sql).await? > 0) })
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
        }
    }

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

    #[test]
    fn oracle_11g_selects_ids_with_rownum_and_skip_locked() {
        let d = Oracle {
            compat: OracleCompat::V11,
        };
        let sql = plan_claim(&d, "zeyn_", "w1", &["mail"], 1_000, 60_000, 2);
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
}
