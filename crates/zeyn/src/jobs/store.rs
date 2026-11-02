//! [`JobStore`]: where queued and scheduled job runs live, with leases so two workers never run
//! the same run, and [`MemoryJobStore`], the default.

use crate::action::{Error, Result};
use crate::ctx::BoxFuture;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Longest payload of a queued run, in bytes. Payloads carry ids and small JSON, not documents.
pub const MAX_JOB_PAYLOAD: usize = 4000;

/// Runs a [`MemoryJobStore`] holds unless built with [`MemoryJobStore::with_capacity`].
pub const MEMORY_JOB_CAPACITY: usize = 10_000;

/// Longest wait between two tries of a failed run.
pub const MAX_BACKOFF_MS: i64 = 600_000;

/// A run to put in the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewJob {
    /// The registered job that runs it.
    pub name: String,
    /// Text the job receives, at most [`MAX_JOB_PAYLOAD`] bytes.
    pub payload: String,
    /// Earliest start, Unix milliseconds.
    pub run_at_ms: i64,
    /// Tries in all, at least one.
    pub max_attempts: u32,
    /// Wait before the second try; it doubles each time up to [`MAX_BACKOFF_MS`].
    pub backoff_ms: i64,
}

/// A run a worker holds a lease on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimedJob {
    pub id: String,
    pub name: String,
    pub payload: String,
    /// Tries so far, counting this one. A worker that stopped during a try counts too.
    pub attempts: u32,
    pub max_attempts: u32,
    pub backoff_ms: i64,
    /// Interval of a scheduled job; `None` for a run queued once.
    pub every_ms: Option<i64>,
}

/// What happens to a run after a try.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextRun {
    /// The run is finished and removed.
    Done,
    /// Back in the queue, due at this time (a retry, or the next run of a schedule).
    Queue { run_at_ms: i64, attempts: u32 },
    /// No tries left: kept as failed for inspection.
    Dead,
}

impl ClaimedJob {
    /// After a successful try: a schedule is due again one interval later, any other run is done.
    pub fn after_success(&self, now_ms: i64) -> NextRun {
        match self.every_ms {
            Some(every) => NextRun::Queue { run_at_ms: now_ms.saturating_add(every), attempts: 0 },
            None => NextRun::Done,
        }
    }

    /// After a failed try: again after the backoff while tries are left, otherwise the next
    /// interval of a schedule, otherwise dead.
    pub fn after_failure(&self, now_ms: i64) -> NextRun {
        if self.attempts < self.max_attempts {
            let shift = self.attempts.saturating_sub(1).min(20);
            let wait = self.backoff_ms.max(0).saturating_mul(1 << shift).min(MAX_BACKOFF_MS);
            return NextRun::Queue { run_at_ms: now_ms.saturating_add(wait), attempts: self.attempts };
        }
        match self.every_ms {
            Some(_) => self.after_success(now_ms),
            None => NextRun::Dead,
        }
    }
}

/// Id of the stored row of a scheduled job.
pub fn schedule_id(name: &str) -> String {
    format!("every:{name}")
}

/// Where job runs are stored. A run is claimed with a lease: until the lease ends no other worker
/// gets it, and a worker that stops without finishing leaves a run that is claimed again once the
/// lease has passed. [`MemoryJobStore`] is the default; `SqlJobStore` (features `postgres`,
/// `oracle`) keeps runs across restarts and shares them between servers.
///
/// `complete` and `fail` only change a run the worker still holds; they return `false` when its
/// lease was lost to another worker.
pub trait JobStore: Send + Sync + 'static {
    /// Queues a run and returns its id.
    fn enqueue(&self, job: NewJob) -> BoxFuture<Result<String>>;

    /// Stores a scheduled job: due first at `first_run_ms` when it is new, keeping its stored next
    /// run when it exists (so a restart does not move it).
    fn schedule(
        &self,
        name: &str,
        every_ms: i64,
        first_run_ms: i64,
        max_attempts: u32,
        backoff_ms: i64,
    ) -> BoxFuture<Result<()>>;

    /// Takes up to `limit` due runs of the named jobs, oldest first: queued runs due by `now_ms`
    /// and running ones whose lease has ended. Each try counts in `attempts`.
    fn claim(
        &self,
        worker: &str,
        names: &[&'static str],
        now_ms: i64,
        lease_ms: i64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<ClaimedJob>>>;

    /// Extends the lease while the run works. `false` when the worker no longer holds it.
    fn heartbeat(&self, id: &str, worker: &str, lease_until_ms: i64) -> BoxFuture<Result<bool>>;

    /// Finishes a successful try (see [`ClaimedJob::after_success`]).
    fn complete(&self, job: &ClaimedJob, worker: &str, now_ms: i64) -> BoxFuture<Result<bool>>;

    /// Finishes a failed try with an error code (see [`ClaimedJob::after_failure`]).
    fn fail(&self, job: &ClaimedJob, worker: &str, error: &'static str, now_ms: i64) -> BoxFuture<Result<bool>>;
}

pub(crate) fn check_new(job: &NewJob) -> Result<()> {
    if job.payload.len() > MAX_JOB_PAYLOAD {
        return Err(Error::Internal(format!("a job payload is at most {MAX_JOB_PAYLOAD} bytes")));
    }
    if job.name.is_empty() || job.name.len() > 64 {
        return Err(Error::Internal("a job name is 1 to 64 bytes".into()));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum State {
    Queued,
    Running,
    Failed,
}

#[derive(Clone, Debug)]
struct Row {
    name: String,
    payload: String,
    state: State,
    run_at: i64,
    attempts: u32,
    max_attempts: u32,
    backoff_ms: i64,
    every_ms: Option<i64>,
    owner: Option<String>,
    lease_until: i64,
    last_error: Option<&'static str>,
}

struct Mem {
    rows: HashMap<String, Row>,
    next_id: u64,
}

/// Job runs in memory: the default store. Runs are lost when the process stops. At most
/// `capacity` runs are held; when it is full the oldest failed run is dropped, and with none to
/// drop `enqueue` fails.
#[derive(Clone)]
pub struct MemoryJobStore {
    inner: Arc<Mutex<Mem>>,
    capacity: usize,
}

impl Default for MemoryJobStore {
    fn default() -> Self {
        MemoryJobStore::with_capacity(MEMORY_JOB_CAPACITY)
    }
}

impl MemoryJobStore {
    /// A store of at most `capacity` runs.
    pub fn with_capacity(capacity: usize) -> Self {
        MemoryJobStore {
            inner: Arc::new(Mutex::new(Mem { rows: HashMap::new(), next_id: 0 })),
            capacity: capacity.max(1),
        }
    }

    /// Runs held now, failed ones included.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.rows.len()).unwrap_or(0)
    }

    /// Whether no run is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Error code of a run kept as failed, `None` when it is not failed.
    pub fn failed(&self, id: &str) -> Option<&'static str> {
        let m = self.inner.lock().ok()?;
        m.rows.get(id).filter(|r| r.state == State::Failed).map(|r| r.last_error.unwrap_or("internal"))
    }

    fn make_room(&self, m: &mut Mem) -> Result<()> {
        if m.rows.len() < self.capacity {
            return Ok(());
        }
        let oldest = m.rows.iter().filter(|(_, r)| r.state == State::Failed).min_by_key(|(_, r)| r.run_at);
        match oldest.map(|(id, _)| id.clone()) {
            Some(id) => {
                m.rows.remove(&id);
                Ok(())
            }
            None => Err(Error::Internal(format!("the job queue is full ({} runs)", self.capacity))),
        }
    }

    fn finish(&self, job: &ClaimedJob, worker: &str, next: NextRun, error: Option<&'static str>) -> Result<bool> {
        let mut m = self.inner.lock().map_err(|_| Error::Internal("job store lock".into()))?;
        let held = m.rows.get(&job.id).is_some_and(|r| r.state == State::Running && r.owner.as_deref() == Some(worker));
        if !held {
            return Ok(false);
        }
        match next {
            NextRun::Done => {
                m.rows.remove(&job.id);
            }
            NextRun::Queue { run_at_ms, attempts } => {
                if let Some(r) = m.rows.get_mut(&job.id) {
                    r.state = State::Queued;
                    r.run_at = run_at_ms;
                    r.attempts = attempts;
                    r.owner = None;
                    r.last_error = error;
                }
            }
            NextRun::Dead => {
                if let Some(r) = m.rows.get_mut(&job.id) {
                    r.state = State::Failed;
                    r.owner = None;
                    r.last_error = error;
                }
            }
        }
        Ok(true)
    }
}

impl JobStore for MemoryJobStore {
    fn enqueue(&self, job: NewJob) -> BoxFuture<Result<String>> {
        let out = (|| {
            check_new(&job)?;
            let mut m = self.inner.lock().map_err(|_| Error::Internal("job store lock".into()))?;
            self.make_room(&mut m)?;
            m.next_id += 1;
            let id = format!("m{}", m.next_id);
            m.rows.insert(
                id.clone(),
                Row {
                    name: job.name,
                    payload: job.payload,
                    state: State::Queued,
                    run_at: job.run_at_ms,
                    attempts: 0,
                    max_attempts: job.max_attempts.max(1),
                    backoff_ms: job.backoff_ms,
                    every_ms: None,
                    owner: None,
                    lease_until: 0,
                    last_error: None,
                },
            );
            Ok(id)
        })();
        Box::pin(async move { out })
    }

    fn schedule(
        &self,
        name: &str,
        every_ms: i64,
        first_run_ms: i64,
        max_attempts: u32,
        backoff_ms: i64,
    ) -> BoxFuture<Result<()>> {
        let out = (|| {
            let mut m = self.inner.lock().map_err(|_| Error::Internal("job store lock".into()))?;
            let id = schedule_id(name);
            if let Some(r) = m.rows.get_mut(&id) {
                r.every_ms = Some(every_ms);
                r.max_attempts = max_attempts.max(1);
                r.backoff_ms = backoff_ms;
                return Ok(());
            }
            self.make_room(&mut m)?;
            m.rows.insert(
                id,
                Row {
                    name: name.to_string(),
                    payload: String::new(),
                    state: State::Queued,
                    run_at: first_run_ms,
                    attempts: 0,
                    max_attempts: max_attempts.max(1),
                    backoff_ms,
                    every_ms: Some(every_ms),
                    owner: None,
                    lease_until: 0,
                    last_error: None,
                },
            );
            Ok(())
        })();
        Box::pin(async move { out })
    }

    fn claim(
        &self,
        worker: &str,
        names: &[&'static str],
        now_ms: i64,
        lease_ms: i64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<ClaimedJob>>> {
        let out = (|| {
            let mut m = self.inner.lock().map_err(|_| Error::Internal("job store lock".into()))?;
            let mut due: Vec<(i64, String)> = m
                .rows
                .iter()
                .filter(|(_, r)| names.contains(&r.name.as_str()))
                .filter(|(_, r)| match r.state {
                    State::Queued => r.run_at <= now_ms,
                    State::Running => r.lease_until < now_ms,
                    State::Failed => false,
                })
                .map(|(id, r)| (r.run_at, id.clone()))
                .collect();
            due.sort_unstable();
            due.truncate(limit);
            let mut claimed = Vec::with_capacity(due.len());
            for (_, id) in due {
                if let Some(r) = m.rows.get_mut(&id) {
                    r.state = State::Running;
                    r.owner = Some(worker.to_string());
                    r.lease_until = now_ms.saturating_add(lease_ms);
                    r.attempts = r.attempts.saturating_add(1);
                    claimed.push(ClaimedJob {
                        id,
                        name: r.name.clone(),
                        payload: r.payload.clone(),
                        attempts: r.attempts,
                        max_attempts: r.max_attempts,
                        backoff_ms: r.backoff_ms,
                        every_ms: r.every_ms,
                    });
                }
            }
            Ok(claimed)
        })();
        Box::pin(async move { out })
    }

    fn heartbeat(&self, id: &str, worker: &str, lease_until_ms: i64) -> BoxFuture<Result<bool>> {
        let out = self.inner.lock().map_err(|_| Error::Internal("job store lock".into())).map(|mut m| {
            match m.rows.get_mut(id) {
                Some(r) if r.state == State::Running && r.owner.as_deref() == Some(worker) => {
                    r.lease_until = lease_until_ms;
                    true
                }
                _ => false,
            }
        });
        Box::pin(async move { out })
    }

    fn complete(&self, job: &ClaimedJob, worker: &str, now_ms: i64) -> BoxFuture<Result<bool>> {
        let out = self.finish(job, worker, job.after_success(now_ms), None);
        Box::pin(async move { out })
    }

    fn fail(&self, job: &ClaimedJob, worker: &str, error: &'static str, now_ms: i64) -> BoxFuture<Result<bool>> {
        let out = self.finish(job, worker, job.after_failure(now_ms), Some(error));
        Box::pin(async move { out })
    }
}
