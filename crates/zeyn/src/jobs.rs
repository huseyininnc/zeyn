//! In process background jobs: interval schedules, triggers from actions, retries with backoff,
//! a cap on jobs running at once, status and graceful shutdown.
//!
//! Without a store, jobs live in the server process: a run that is waiting or running when the
//! process stops is lost. With a durable [`JobStore`] (`SqlJobStore`, features `postgres` and
//! `oracle`) queued runs and the next run of every schedule are stored: they survive restarts,
//! servers share them, and a lease makes sure only one worker runs a run at a time.
//!
//! ```ignore
//! App::new("Billing")
//!     .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
//!     .job("rebuild_index", Schedule::manual(), rebuild_index)
//!
//! async fn send_reminders(ctx: Ctx) -> Result<()> { ... }
//!
//! #[action(permission = "search.admin")]
//! pub async fn reindex(_: (), jobs: Jobs) -> Result<()> { jobs.trigger("rebuild_index") }
//!
//! App::new("Billing")
//!     .job_store(SqlJobStore::new(db))
//!     .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
//!
//! async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
//! jobs.enqueue("send_invoice", "42").await?;
//! ```

#[cfg(any(feature = "postgres", feature = "oracle"))]
mod sql;
mod store;

#[cfg(any(feature = "postgres", feature = "oracle"))]
pub use sql::{SqlJobStore, create_job_tables};
pub use store::{
    ClaimedJob, JobStore, MAX_BACKOFF_MS, MAX_JOB_PAYLOAD, MEMORY_JOB_CAPACITY, MemoryJobStore, NewJob, NextRun,
};

use crate::action::{Error, Result};
use crate::ctx::{BoxFuture, Ctx, Service};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::{Notify, Semaphore};

/// When a job runs and how often a failed run is tried again.
#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    every: Option<Duration>,
    retries: u32,
    backoff: Duration,
}

impl Schedule {
    /// Runs every `interval`, the first time one interval after start, and when triggered.
    pub fn every(interval: Duration) -> Self {
        Schedule { every: Some(interval.max(Duration::from_millis(10))), retries: 0, backoff: Duration::from_secs(1) }
    }

    /// Runs only when triggered with [`Jobs::trigger`].
    pub fn manual() -> Self {
        Schedule { every: None, retries: 0, backoff: Duration::from_secs(1) }
    }

    /// Tries a failed run up to `times` more times, waiting `first_delay`, then twice as long
    /// each time, at most ten minutes.
    pub fn retry(mut self, times: u32, first_delay: Duration) -> Self {
        self.retries = times;
        self.backoff = first_delay;
        self
    }
}

/// What a job is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Idle,
    Waiting,
    Running,
    Retrying,
    Succeeded,
    Failed,
}

/// Status of one job. Errors are shown by code only; details go to the server log.
#[derive(Clone, Debug, Serialize)]
pub struct JobStatus {
    pub name: &'static str,
    pub state: JobState,
    pub runs: u64,
    pub failures: u64,
    pub last_started_ms: Option<i64>,
    pub last_finished_ms: Option<i64>,
    pub last_error: Option<&'static str>,
    pub every_ms: Option<u128>,
}

/// How long a worker holds a claimed run without a heartbeat. It renews the lease every third of
/// this while the run works; a run whose worker stopped is claimed again once it has passed.
pub const JOB_LEASE: Duration = Duration::from_secs(60);

/// How often a worker looks for due runs in the store when nothing wakes it earlier.
pub const JOB_POLL: Duration = Duration::from_secs(1);

type JobFn = Arc<dyn Fn(Ctx, String) -> BoxFuture<Result<()>> + Send + Sync>;
type CtxFn = Arc<dyn Fn(&'static str) -> Ctx + Send + Sync>;

struct Job {
    name: &'static str,
    schedule: Schedule,
    run: JobFn,
    wake: Notify,
}

struct Inner {
    jobs: Mutex<Vec<Arc<Job>>>,
    status: Mutex<HashMap<&'static str, JobStatus>>,
    slots: Arc<Semaphore>,
    stopping: AtomicBool,
    stop: Notify,
    running: AtomicUsize,
    started: AtomicBool,
    store: OnceLock<Arc<dyn JobStore>>,
    durable: AtomicBool,
    worker: String,
    queued: Notify,
}

/// The app's background jobs. Inject it into actions to trigger jobs or read their status.
#[derive(Clone)]
pub struct Jobs(Arc<Inner>);

impl Service for Jobs {}

impl Default for Jobs {
    fn default() -> Self {
        Jobs::with_limit(4)
    }
}

fn now_ms() -> i64 {
    crate::time::DateTime::now().unix_ms()
}

/// A random id of 32 hexadecimal digits, for runs and workers.
pub(crate) fn random_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("the operating system random source must work");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

impl Jobs {
    /// Jobs of which at most `limit` run at the same time.
    pub fn with_limit(limit: usize) -> Self {
        Jobs(Arc::new(Inner {
            jobs: Mutex::new(Vec::new()),
            status: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(limit.max(1))),
            stopping: AtomicBool::new(false),
            stop: Notify::new(),
            running: AtomicUsize::new(0),
            started: AtomicBool::new(false),
            store: OnceLock::new(),
            durable: AtomicBool::new(false),
            worker: random_id(),
            queued: Notify::new(),
        }))
    }

    /// Keeps queued runs and the next run of every scheduled job in `store`, such as a
    /// `SqlJobStore`, instead of in memory. Set it once, before the jobs start.
    pub fn set_store(&self, store: impl JobStore) -> Result<()> {
        if self.0.started.load(Ordering::SeqCst) {
            return Err(Error::Internal("set the job store before the jobs start".into()));
        }
        self.0.store.set(Arc::new(store)).map_err(|_| Error::Internal("the job store is already set".into()))?;
        self.0.durable.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn store(&self) -> &Arc<dyn JobStore> {
        self.0.store.get_or_init(|| Arc::new(MemoryJobStore::default()))
    }

    fn durable(&self) -> bool {
        self.0.durable.load(Ordering::SeqCst)
    }

    /// Registers a job that receives the payload of each queued run (see [`Jobs::enqueue`]).
    /// Scheduled and triggered runs get an empty payload.
    pub fn add_queued<F, Fut>(&self, name: &'static str, schedule: Schedule, run: F)
    where
        F: Fn(Ctx, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.register(name, schedule, Arc::new(move |ctx, payload| Box::pin(run(ctx, payload))));
    }

    /// Queues a run of a registered job with `payload` (at most [`MAX_JOB_PAYLOAD`] bytes) and
    /// returns its id. The run is tried as the job's [`Schedule::retry`] says; with a durable store
    /// it survives restarts.
    pub async fn enqueue(&self, name: &str, payload: impl Into<String>) -> Result<String> {
        self.enqueue_after(name, payload, Duration::ZERO).await
    }

    /// Queues a run that starts no earlier than `delay` from now (see [`Jobs::enqueue`]).
    pub async fn enqueue_after(&self, name: &str, payload: impl Into<String>, delay: Duration) -> Result<String> {
        if self.0.stopping.load(Ordering::SeqCst) {
            return Err(Error::Conflict);
        }
        let job = self.new_job(name, payload.into(), delay)?;
        let id = self.store().enqueue(job).await?;
        self.0.queued.notify_one();
        Ok(id)
    }

    fn new_job(&self, name: &str, payload: String, delay: Duration) -> Result<NewJob> {
        let jobs = self.0.jobs.lock().map_err(|_| Error::Internal("jobs lock".into()))?;
        let job = jobs.iter().find(|j| j.name == name).ok_or(Error::NotFound)?;
        Ok(NewJob {
            name: job.name.to_string(),
            payload,
            run_at_ms: now_ms().saturating_add(ms(delay)),
            max_attempts: job.schedule.retries.saturating_add(1),
            backoff_ms: ms(job.schedule.backoff),
        })
    }

    /// Registers a job. Names must be unique; a second job with the same name is ignored.
    pub fn add<F, Fut>(&self, name: &'static str, schedule: Schedule, run: F)
    where
        F: Fn(Ctx) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        self.register(name, schedule, Arc::new(move |ctx, _| Box::pin(run(ctx))));
    }

    fn register(&self, name: &'static str, schedule: Schedule, run: JobFn) {
        let Ok(mut jobs) = self.0.jobs.lock() else { return };
        if jobs.iter().any(|j| j.name == name) {
            return;
        }
        jobs.push(Arc::new(Job { name, schedule, run, wake: Notify::new() }));
        if let Ok(mut status) = self.0.status.lock() {
            status.insert(
                name,
                JobStatus {
                    name,
                    state: JobState::Idle,
                    runs: 0,
                    failures: 0,
                    last_started_ms: None,
                    last_finished_ms: None,
                    last_error: None,
                    every_ms: schedule.every.map(|d| d.as_millis()),
                },
            );
        }
    }

    /// Asks a job to run as soon as a slot is free. A job triggered while it runs runs once more
    /// afterwards. With a durable store the run is queued there (empty payload), so any server
    /// may take it.
    pub fn trigger(&self, name: &str) -> Result<()> {
        if self.0.stopping.load(Ordering::SeqCst) {
            return Err(Error::Conflict);
        }
        if self.durable() {
            let job = self.new_job(name, String::new(), Duration::ZERO)?;
            let handle = tokio::runtime::Handle::try_current()
                .map_err(|_| Error::Internal("trigger durable jobs on a Tokio runtime".into()))?;
            let me = self.clone();
            handle.spawn(async move {
                match me.store().enqueue(job).await {
                    Ok(_) => me.0.queued.notify_one(),
                    Err(e) => tracing::error!("queueing a triggered job failed: {e:?}"),
                }
            });
            return Ok(());
        }
        let jobs = self.0.jobs.lock().map_err(|_| Error::Internal("jobs lock".into()))?;
        let job = jobs.iter().find(|j| j.name == name).ok_or(Error::NotFound)?;
        job.wake.notify_one();
        self.set(job.name, |s| {
            if s.state != JobState::Running {
                s.state = JobState::Waiting;
            }
        });
        Ok(())
    }

    /// Status of every job, in registration order.
    pub fn status(&self) -> Vec<JobStatus> {
        let names: Vec<&'static str> =
            self.0.jobs.lock().map(|j| j.iter().map(|j| j.name).collect()).unwrap_or_default();
        let status = self.0.status.lock();
        names.iter().filter_map(|n| status.as_ref().ok().and_then(|s| s.get(n).cloned())).collect()
    }

    fn set(&self, name: &'static str, change: impl FnOnce(&mut JobStatus)) {
        if let Ok(mut status) = self.0.status.lock() {
            if let Some(s) = status.get_mut(name) {
                change(s);
            }
        }
    }

    /// Whether no job is registered and no store is set.
    pub(crate) fn is_empty(&self) -> bool {
        !self.durable() && self.0.jobs.lock().map(|j| j.is_empty()).unwrap_or(true)
    }

    /// Starts the schedulers on the current Tokio runtime, once; `ctx` builds the context of each
    /// run from the job name. `App` does this when it builds its router.
    pub fn start(&self, ctx: impl Fn(&'static str) -> Ctx + Send + Sync + 'static) {
        let ctx: CtxFn = Arc::new(ctx);
        if self.0.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let jobs: Vec<Arc<Job>> = self.0.jobs.lock().map(|j| j.clone()).unwrap_or_default();
        if jobs.is_empty() {
            return;
        }
        let durable = self.durable();
        let mut every = Vec::new();
        for job in &jobs {
            match job.schedule.every {
                Some(d) if durable => every.push((job.name, ms(d), job.schedule)),
                _ if durable => {}
                _ => {
                    let me = self.clone();
                    let (job, ctx) = (job.clone(), ctx.clone());
                    tokio::spawn(async move { me.schedule(job, ctx).await });
                }
            }
        }
        let names: Vec<&'static str> = jobs.iter().map(|j| j.name).collect();
        let me = self.clone();
        tokio::spawn(async move {
            for (name, every_ms, schedule) in every {
                let first = now_ms().saturating_add(every_ms);
                let retries = schedule.retries.saturating_add(1);
                if let Err(e) = me.store().schedule(name, every_ms, first, retries, ms(schedule.backoff)).await {
                    tracing::error!(job = name, "storing the schedule of job `{name}` failed: {e:?}");
                }
            }
            me.work(names, ctx).await
        });
    }

    /// Claims due runs from the store while slots are free, and runs each with a lease.
    async fn work(self, names: Vec<&'static str>, ctx: CtxFn) {
        loop {
            let first = tokio::select! {
                p = self.0.slots.clone().acquire_owned() => p,
                _ = self.0.stop.notified() => return,
            };
            let Ok(first) = first else { return };
            if self.0.stopping.load(Ordering::SeqCst) {
                return;
            }
            let mut permits = vec![first];
            while let Ok(p) = self.0.slots.clone().try_acquire_owned() {
                permits.push(p);
            }
            let wanted = permits.len();
            let claimed = match self.store().claim(&self.0.worker, &names, now_ms(), ms(JOB_LEASE), wanted).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("claiming jobs failed: {e:?}");
                    Vec::new()
                }
            };
            let full = claimed.len() >= wanted;
            for job in claimed {
                let Some(permit) = permits.pop() else { break };
                let (me, ctx) = (self.clone(), ctx.clone());
                self.0.running.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    me.run_claimed(job, &ctx).await;
                    me.0.running.fetch_sub(1, Ordering::SeqCst);
                    drop(permit);
                });
            }
            drop(permits);
            if !full {
                tokio::select! {
                    _ = tokio::time::sleep(JOB_POLL) => {}
                    _ = self.0.queued.notified() => {}
                    _ = self.0.stop.notified() => return,
                }
            }
        }
    }

    /// One try of a claimed run: the lease is renewed while it works and the outcome is stored.
    /// A run whose lease was lost to another worker is dropped here.
    async fn run_claimed(&self, job: ClaimedJob, ctx: &CtxFn) {
        let store = self.store().clone();
        let worker = self.0.worker.as_str();
        let found = self.0.jobs.lock().ok().and_then(|j| j.iter().find(|j| j.name == job.name).cloned());
        let Some(handler) = found else {
            let _ = store.fail(&job, worker, "not_found", now_ms()).await;
            return;
        };
        if job.attempts > job.max_attempts {
            let _ = store.fail(&job, worker, "abandoned", now_ms()).await;
            return;
        }
        self.set(handler.name, |s| {
            s.state = JobState::Running;
            s.runs += 1;
            s.last_started_ms = Some(now_ms());
        });
        let run = (handler.run)(ctx(handler.name), job.payload.clone());
        let beat = async {
            loop {
                tokio::time::sleep(JOB_LEASE / 3).await;
                match store.heartbeat(&job.id, worker, now_ms().saturating_add(ms(JOB_LEASE))).await {
                    Ok(false) => return,
                    Ok(true) => {}
                    Err(e) => tracing::warn!(job = handler.name, "job heartbeat failed: {e:?}"),
                }
            }
        };
        let result = tokio::select! {
            r = run => Some(r),
            _ = beat => None,
        };
        let finished = now_ms();
        match result {
            None => {
                tracing::warn!(job = handler.name, "job `{}` lost its lease and was stopped", handler.name);
                self.set(handler.name, |s| s.state = JobState::Waiting);
            }
            Some(Ok(())) => {
                if let Err(e) = store.complete(&job, worker, finished).await {
                    tracing::error!(job = handler.name, "storing a finished job failed: {e:?}");
                }
                self.set(handler.name, |s| {
                    s.state = JobState::Succeeded;
                    s.last_finished_ms = Some(finished);
                    s.last_error = None;
                });
            }
            Some(Err(e)) => {
                if let Error::Internal(detail) = &e {
                    tracing::error!(job = handler.name, "job `{}` failed: {detail}", handler.name);
                }
                if let Err(e) = store.fail(&job, worker, e.code(), finished).await {
                    tracing::error!(job = handler.name, "storing a failed job failed: {e:?}");
                }
                let retry = job.attempts < job.max_attempts;
                self.set(handler.name, |s| {
                    s.failures += 1;
                    s.last_finished_ms = Some(finished);
                    s.last_error = Some(e.code());
                    s.state = if retry { JobState::Retrying } else { JobState::Failed };
                });
            }
        }
    }

    async fn schedule(self, job: Arc<Job>, ctx: CtxFn) {
        loop {
            let tick = async {
                match job.schedule.every {
                    Some(d) => tokio::time::sleep(d).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::select! {
                _ = tick => {}
                _ = job.wake.notified() => {}
                _ = self.0.stop.notified() => return,
            }
            if self.0.stopping.load(Ordering::SeqCst) {
                return;
            }
            let Ok(permit) = self.0.slots.acquire().await else { return };
            if self.0.stopping.load(Ordering::SeqCst) {
                return;
            }
            self.0.running.fetch_add(1, Ordering::SeqCst);
            self.run_once(&job, ctx(job.name)).await;
            self.0.running.fetch_sub(1, Ordering::SeqCst);
            drop(permit);
        }
    }

    async fn run_once(&self, job: &Job, ctx: Ctx) {
        let mut delay = job.schedule.backoff;
        for attempt in 0..=job.schedule.retries {
            self.set(job.name, |s| {
                s.state = JobState::Running;
                s.runs += 1;
                s.last_started_ms = Some(now_ms());
            });
            let result = (job.run)(ctx.clone(), String::new()).await;
            let finished = now_ms();
            match result {
                Ok(()) => {
                    self.set(job.name, |s| {
                        s.state = JobState::Succeeded;
                        s.last_finished_ms = Some(finished);
                        s.last_error = None;
                    });
                    return;
                }
                Err(e) => {
                    if let Error::Internal(detail) = &e {
                        tracing::error!(job = job.name, "job `{}` failed: {detail}", job.name);
                    }
                    let last = attempt == job.schedule.retries || self.0.stopping.load(Ordering::SeqCst);
                    self.set(job.name, |s| {
                        s.failures += 1;
                        s.last_finished_ms = Some(finished);
                        s.last_error = Some(e.code());
                        s.state = if last { JobState::Failed } else { JobState::Retrying };
                    });
                    if last {
                        return;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = self.0.stop.notified() => {
                            self.set(job.name, |s| s.state = JobState::Failed);
                            return;
                        }
                    }
                    delay = (delay * 2).min(Duration::from_secs(600));
                }
            }
        }
    }

    /// Stops scheduling, cancels retry waits and waits up to `grace` for running jobs to finish.
    /// Returns whether every job finished in time.
    pub async fn shutdown(&self, grace: Duration) -> bool {
        self.0.stopping.store(true, Ordering::SeqCst);
        self.0.stop.notify_waiters();
        self.0.slots.close();
        let deadline = tokio::time::Instant::now() + grace;
        while self.0.running.load(Ordering::SeqCst) > 0 {
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.0.stop.notify_waiters();
        }
        true
    }
}
