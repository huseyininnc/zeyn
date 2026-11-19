//! [`JobStore`]: where queued and scheduled job runs live, with leases so two workers never run
//! the same run, and [`MemoryJobStore`], the default.

use crate::action::{Error, Result};
use crate::ctx::BoxFuture;
#[cfg(feature = "metronome")]
use crate::metronome::{RunRow, TriggerRow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Longest payload of a queued run, in bytes. Payloads carry ids and small JSON, not documents.
pub const MAX_JOB_PAYLOAD: usize = 4000;

/// Runs a [`MemoryJobStore`] holds unless built with [`MemoryJobStore::with_capacity`].
pub const MEMORY_JOB_CAPACITY: usize = 10_000;

/// Longest wait between two tries of a failed run.
pub const MAX_BACKOFF_MS: i64 = 600_000;

/// A run to put in the queue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// The trigger that queued it: an id, `manual` or `chain`; `None` for `Jobs::enqueue`.
    pub trigger_id: Option<String>,
    /// Unique per trigger and tick, so a tick is queued once across servers.
    pub tick_key: Option<String>,
    /// The tick the run is for, Unix milliseconds.
    pub scheduled_ms: Option<i64>,
    /// Claim order between due runs, higher first.
    pub priority: i16,
    /// Longest time one try may take, in milliseconds.
    pub timeout_ms: Option<i64>,
    /// Who asked for a manual run.
    pub started_by: Option<String>,
}

/// A run a worker holds a lease on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// See [`NewJob::trigger_id`].
    pub trigger_id: Option<String>,
    /// See [`NewJob::scheduled_ms`].
    pub scheduled_ms: Option<i64>,
    /// See [`NewJob::timeout_ms`].
    pub timeout_ms: Option<i64>,
    /// See [`NewJob::started_by`].
    pub started_by: Option<String>,
    /// When this try was claimed, Unix milliseconds.
    pub started_ms: i64,
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
            Some(every) => NextRun::Queue {
                run_at_ms: now_ms.saturating_add(every),
                attempts: 0,
            },
            None => NextRun::Done,
        }
    }

    /// After a failed try: again after the backoff while tries are left, otherwise the next
    /// interval of a schedule, otherwise dead.
    pub fn after_failure(&self, now_ms: i64) -> NextRun {
        if self.attempts < self.max_attempts {
            let shift = self.attempts.saturating_sub(1).min(20);
            let wait = self
                .backoff_ms
                .max(0)
                .saturating_mul(1 << shift)
                .min(MAX_BACKOFF_MS);
            return NextRun::Queue {
                run_at_ms: now_ms.saturating_add(wait),
                attempts: self.attempts,
            };
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
    fn fail(
        &self,
        job: &ClaimedJob,
        worker: &str,
        error: &'static str,
        now_ms: i64,
    ) -> BoxFuture<Result<bool>>;

    /// Stores a trigger and returns it as stored. A new trigger is inserted as given; a known one
    /// keeps its next tick and pause flag unless its expression or time zone changed, which
    /// takes the given next tick (feature `metronome`).
    #[cfg(feature = "metronome")]
    fn add_trigger(&self, row: TriggerRow) -> BoxFuture<Result<TriggerRow>> {
        let _ = row;
        unsupported()
    }

    /// Every stored trigger.
    #[cfg(feature = "metronome")]
    fn triggers(&self) -> BoxFuture<Result<Vec<TriggerRow>>> {
        unsupported()
    }

    /// In one transaction: moves the trigger's next tick from `from_ms` to `to_ms` and queues
    /// `runs` and the `skipped` history row. `None` when it did; when another server moved the
    /// tick first nothing is queued and the stored next tick is returned.
    #[cfg(feature = "metronome")]
    fn fire(
        &self,
        job: &str,
        trigger: &str,
        from_ms: i64,
        to_ms: i64,
        runs: Vec<NewJob>,
        skipped: Option<RunRow>,
    ) -> BoxFuture<Result<Option<i64>>> {
        let _ = (job, trigger, from_ms, to_ms, runs, skipped);
        unsupported()
    }

    /// Sets the next tick of a trigger.
    #[cfg(feature = "metronome")]
    fn move_trigger(&self, job: &str, trigger: &str, next_ms: i64) -> BoxFuture<Result<()>> {
        let _ = (job, trigger, next_ms);
        unsupported()
    }

    /// Pauses or resumes a job (`trigger` is `None`) or one of its triggers.
    #[cfg(feature = "metronome")]
    fn set_paused(
        &self,
        job: &str,
        trigger: Option<&str>,
        paused: bool,
        now_ms: i64,
    ) -> BoxFuture<Result<()>> {
        let _ = (job, trigger, paused, now_ms);
        unsupported()
    }

    /// Names of paused jobs.
    #[cfg(feature = "metronome")]
    fn paused_jobs(&self) -> BoxFuture<Result<Vec<String>>> {
        unsupported()
    }

    /// History rows, newest first, of one job or all.
    #[cfg(feature = "metronome")]
    fn history(
        &self,
        job: Option<&str>,
        offset: u64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<RunRow>>> {
        let _ = (job, offset, limit);
        unsupported()
    }

    /// Deletes at most `limit` history rows that ended before `before_ms` or fall outside the
    /// newest `keep`, and returns how many it deleted.
    #[cfg(feature = "metronome")]
    fn prune_history(&self, before_ms: i64, keep: u64, limit: usize) -> BoxFuture<Result<u64>> {
        let _ = (before_ms, keep, limit);
        unsupported()
    }
}

#[cfg(feature = "metronome")]
fn unsupported<T: Send + 'static>() -> BoxFuture<Result<T>> {
    Box::pin(async {
        Err(Error::Internal(
            "this job store does not keep triggers; use MemoryJobStore or SqlJobStore with the feature `metronome`".into(),
        ))
    })
}

/// The history row of a finished try (feature `metronome`).
#[cfg(feature = "metronome")]
pub(crate) fn run_row(
    job: &ClaimedJob,
    worker: &str,
    error: Option<&'static str>,
    now_ms: i64,
) -> RunRow {
    RunRow {
        id: super::random_id(),
        run_id: job.id.clone(),
        job: job.name.clone(),
        trigger_id: job.trigger_id.clone(),
        scheduled_ms: job.scheduled_ms,
        started_ms: job.started_ms,
        ended_ms: now_ms,
        attempt: job.attempts,
        outcome: match error {
            None => "succeeded",
            Some("timeout") => "timeout",
            Some(_) => "failed",
        }
        .into(),
        error_code: error.map(str::to_string),
        worker: worker.into(),
        started_by: job.started_by.clone(),
    }
}

/// Claim order: higher priority first, oldest first within one; a run waiting longer than
/// `PRIORITY_AGING` counts as the highest priority.
fn claim_key(r: &Row, now_ms: i64) -> (std::cmp::Reverse<i16>, i64) {
    #[cfg(feature = "metronome")]
    let aged = r.run_at < now_ms.saturating_sub(super::ms(crate::metronome::PRIORITY_AGING));
    #[cfg(not(feature = "metronome"))]
    let aged = now_ms == i64::MIN;
    (
        std::cmp::Reverse(if aged { i16::MAX } else { r.priority }),
        r.run_at,
    )
}

pub(crate) fn check_new(job: &NewJob) -> Result<()> {
    if job.payload.len() > MAX_JOB_PAYLOAD {
        return Err(Error::Internal(format!(
            "a job payload is at most {MAX_JOB_PAYLOAD} bytes"
        )));
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
    trigger_id: Option<String>,
    scheduled_ms: Option<i64>,
    priority: i16,
    timeout_ms: Option<i64>,
    started_by: Option<String>,
}

impl Row {
    fn of(job: NewJob) -> Row {
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
            trigger_id: job.trigger_id,
            scheduled_ms: job.scheduled_ms,
            priority: job.priority,
            timeout_ms: job.timeout_ms,
            started_by: job.started_by,
        }
    }
}

#[derive(Default)]
struct Mem {
    rows: HashMap<String, Row>,
    next_id: u64,
    #[cfg(feature = "metronome")]
    triggers: Vec<TriggerRow>,
    #[cfg(feature = "metronome")]
    paused: Vec<String>,
    #[cfg(feature = "metronome")]
    history: std::collections::VecDeque<RunRow>,
}

/// Job runs in memory: the default store. Runs are lost when the process stops. At most
/// `capacity` runs are held; when it is full the oldest failed run is dropped, and with none to
/// drop `enqueue` fails. With the feature `metronome` it also keeps triggers, pauses and the
/// newest `MEMORY_HISTORY_CAPACITY` history rows.
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
            inner: Arc::new(Mutex::new(Mem::default())),
            capacity: capacity.max(1),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Mem>> {
        self.inner
            .lock()
            .map_err(|_| Error::Internal("job store lock".into()))
    }

    /// Runs held now, failed ones included.
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .map(|m| m.rows.len())
            .unwrap_or(0)
    }

    /// Whether no run is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Error code of a run kept as failed, `None` when it is not failed.
    pub fn failed(&self, id: &str) -> Option<&'static str> {
        let m = self.inner.lock().ok()?;
        m.rows
            .get(id)
            .filter(|r| r.state == State::Failed)
            .map(|r| r.last_error.unwrap_or("internal"))
    }

    fn make_room(&self, m: &mut Mem) -> Result<()> {
        if m.rows.len() < self.capacity {
            return Ok(());
        }
        let oldest = m
            .rows
            .iter()
            .filter(|(_, r)| r.state == State::Failed)
            .min_by_key(|(_, r)| r.run_at);
        match oldest.map(|(id, _)| id.clone()) {
            Some(id) => {
                m.rows.remove(&id);
                Ok(())
            }
            None => Err(Error::Internal(format!(
                "the job queue is full ({} runs)",
                self.capacity
            ))),
        }
    }

    fn insert(&self, m: &mut Mem, job: NewJob) -> Result<String> {
        check_new(&job)?;
        self.make_room(m)?;
        m.next_id += 1;
        let id = format!("m{}", m.next_id);
        m.rows.insert(id.clone(), Row::of(job));
        Ok(id)
    }

    fn finish(
        &self,
        job: &ClaimedJob,
        worker: &str,
        next: NextRun,
        error: Option<&'static str>,
        now_ms: i64,
    ) -> Result<bool> {
        let mut m = self.lock()?;
        let held = m
            .rows
            .get(&job.id)
            .is_some_and(|r| r.state == State::Running && r.owner.as_deref() == Some(worker));
        if !held {
            return Ok(false);
        }
        match next {
            NextRun::Done => {
                m.rows.remove(&job.id);
            }
            NextRun::Queue {
                run_at_ms,
                attempts,
            } => {
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
        #[cfg(feature = "metronome")]
        push_history(&mut m, run_row(job, worker, error, now_ms));
        #[cfg(not(feature = "metronome"))]
        let _ = now_ms;
        Ok(true)
    }
}

#[cfg(feature = "metronome")]
fn push_history(m: &mut Mem, row: RunRow) {
    if m.history.len() >= crate::metronome::MEMORY_HISTORY_CAPACITY {
        m.history.pop_front();
    }
    m.history.push_back(row);
}

impl JobStore for MemoryJobStore {
    fn enqueue(&self, job: NewJob) -> BoxFuture<Result<String>> {
        let out = self
            .lock()
            .and_then(|mut m| self.insert(&mut m, job));
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
            let mut m = self.lock()?;
            let id = schedule_id(name);
            if let Some(r) = m.rows.get_mut(&id) {
                r.every_ms = Some(every_ms);
                r.max_attempts = max_attempts.max(1);
                r.backoff_ms = backoff_ms;
                return Ok(());
            }
            self.make_room(&mut m)?;
            let mut row = Row::of(NewJob {
                name: name.to_string(),
                run_at_ms: first_run_ms,
                max_attempts,
                backoff_ms,
                ..NewJob::default()
            });
            row.every_ms = Some(every_ms);
            m.rows.insert(id, row);
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
            let mut m = self.lock()?;
            let mut due: Vec<((std::cmp::Reverse<i16>, i64), String)> = m
                .rows
                .iter()
                .filter(|(_, r)| names.contains(&r.name.as_str()))
                .filter(|(_, r)| match r.state {
                    State::Queued => r.run_at <= now_ms,
                    State::Running => r.lease_until < now_ms,
                    State::Failed => false,
                })
                .map(|(id, r)| (claim_key(r, now_ms), id.clone()))
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
                        trigger_id: r.trigger_id.clone(),
                        scheduled_ms: r.scheduled_ms,
                        timeout_ms: r.timeout_ms,
                        started_by: r.started_by.clone(),
                        started_ms: now_ms,
                    });
                }
            }
            Ok(claimed)
        })();
        Box::pin(async move { out })
    }

    fn heartbeat(&self, id: &str, worker: &str, lease_until_ms: i64) -> BoxFuture<Result<bool>> {
        let out = self
            .lock()
            .map(|mut m| match m.rows.get_mut(id) {
                Some(r) if r.state == State::Running && r.owner.as_deref() == Some(worker) => {
                    r.lease_until = lease_until_ms;
                    true
                }
                _ => false,
            });
        Box::pin(async move { out })
    }

    fn complete(&self, job: &ClaimedJob, worker: &str, now_ms: i64) -> BoxFuture<Result<bool>> {
        let out = self.finish(job, worker, job.after_success(now_ms), None, now_ms);
        Box::pin(async move { out })
    }

    fn fail(
        &self,
        job: &ClaimedJob,
        worker: &str,
        error: &'static str,
        now_ms: i64,
    ) -> BoxFuture<Result<bool>> {
        let out = self.finish(job, worker, job.after_failure(now_ms), Some(error), now_ms);
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn add_trigger(&self, row: TriggerRow) -> BoxFuture<Result<TriggerRow>> {
        let out = self.lock().map(|mut m| {
            match m
                .triggers
                .iter_mut()
                .find(|t| t.job == row.job && t.id == row.id)
            {
                Some(t) => {
                    let moved = t.expr != row.expr || t.time_zone != row.time_zone;
                    let (next, paused) = (t.next_ms, t.paused);
                    *t = row;
                    t.paused = paused;
                    if !moved {
                        t.next_ms = next;
                    }
                    t.clone()
                }
                None => {
                    m.triggers.push(row.clone());
                    row
                }
            }
        });
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn triggers(&self) -> BoxFuture<Result<Vec<TriggerRow>>> {
        let out = self.lock().map(|m| m.triggers.clone());
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn fire(
        &self,
        job: &str,
        trigger: &str,
        from_ms: i64,
        to_ms: i64,
        runs: Vec<NewJob>,
        skipped: Option<RunRow>,
    ) -> BoxFuture<Result<Option<i64>>> {
        let out = (|| {
            let mut m = self.lock()?;
            let stored = m
                .triggers
                .iter()
                .position(|t| t.job == job && t.id == trigger)
                .ok_or(Error::NotFound)?;
            let next = m.triggers[stored].next_ms;
            if next != from_ms {
                return Ok(Some(next));
            }
            if m.rows.len() + runs.len() > self.capacity {
                return Err(Error::Internal(format!(
                    "the job queue is full ({} runs)",
                    self.capacity
                )));
            }
            for run in runs {
                self.insert(&mut m, run)?;
            }
            if let Some(row) = skipped {
                push_history(&mut m, row);
            }
            m.triggers[stored].next_ms = to_ms;
            Ok(None)
        })();
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn move_trigger(&self, job: &str, trigger: &str, next_ms: i64) -> BoxFuture<Result<()>> {
        let out = self.lock().map(|mut m| {
            for t in m
                .triggers
                .iter_mut()
                .filter(|t| t.job == job && t.id == trigger)
            {
                t.next_ms = next_ms;
            }
        });
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn set_paused(
        &self,
        job: &str,
        trigger: Option<&str>,
        paused: bool,
        _now_ms: i64,
    ) -> BoxFuture<Result<()>> {
        let out = self.lock().map(|mut m| match trigger {
            Some(id) => {
                for t in m
                    .triggers
                    .iter_mut()
                    .filter(|t| t.job == job && t.id == id)
                {
                    t.paused = paused;
                }
            }
            None => {
                m.paused.retain(|j| j != job);
                if paused {
                    m.paused.push(job.into());
                }
            }
        });
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn paused_jobs(&self) -> BoxFuture<Result<Vec<String>>> {
        let out = self.lock().map(|m| m.paused.clone());
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn history(
        &self,
        job: Option<&str>,
        offset: u64,
        limit: usize,
    ) -> BoxFuture<Result<Vec<RunRow>>> {
        let out = self.lock().map(|m| {
            m.history
                .iter()
                .rev()
                .filter(|r| job.is_none_or(|j| r.job == j))
                .skip(usize::try_from(offset).unwrap_or(usize::MAX))
                .take(limit)
                .cloned()
                .collect()
        });
        Box::pin(async move { out })
    }

    #[cfg(feature = "metronome")]
    fn prune_history(&self, before_ms: i64, keep: u64, limit: usize) -> BoxFuture<Result<u64>> {
        let out = self.lock().map(|mut m| {
            let keep = usize::try_from(keep).unwrap_or(usize::MAX);
            let mut deleted = 0;
            while deleted < limit
                && m.history
                    .front()
                    .is_some_and(|r| r.ended_ms < before_ms || m.history.len() > keep)
            {
                m.history.pop_front();
                deleted += 1;
            }
            deleted as u64
        });
        Box::pin(async move { out })
    }
}
