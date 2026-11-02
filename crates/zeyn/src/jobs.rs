//! In process background jobs: interval schedules, triggers from actions, retries with backoff,
//! a cap on jobs running at once, status and graceful shutdown.
//!
//! Jobs live in the server process. A job that is waiting or running when the process stops is
//! not run again on the next start; jobs that must survive restarts need a database backed queue,
//! which comes with the data layer.
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
//! ```

use crate::action::{Error, Result};
use crate::ctx::{BoxFuture, Ctx, Service};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
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

type JobFn = Arc<dyn Fn(Ctx) -> BoxFuture<Result<()>> + Send + Sync>;

struct Job {
    name: &'static str,
    schedule: Schedule,
    run: JobFn,
    wake: Notify,
}

struct Inner {
    jobs: Mutex<Vec<Arc<Job>>>,
    status: Mutex<HashMap<&'static str, JobStatus>>,
    slots: Semaphore,
    stopping: AtomicBool,
    stop: Notify,
    running: AtomicUsize,
    started: AtomicBool,
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

impl Jobs {
    /// Jobs of which at most `limit` run at the same time.
    pub fn with_limit(limit: usize) -> Self {
        Jobs(Arc::new(Inner {
            jobs: Mutex::new(Vec::new()),
            status: Mutex::new(HashMap::new()),
            slots: Semaphore::new(limit.max(1)),
            stopping: AtomicBool::new(false),
            stop: Notify::new(),
            running: AtomicUsize::new(0),
            started: AtomicBool::new(false),
        }))
    }

    /// Registers a job. Names must be unique; a second job with the same name is ignored.
    pub fn add<F, Fut>(&self, name: &'static str, schedule: Schedule, run: F)
    where
        F: Fn(Ctx) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let Ok(mut jobs) = self.0.jobs.lock() else { return };
        if jobs.iter().any(|j| j.name == name) {
            return;
        }
        jobs.push(Arc::new(Job { name, schedule, run: Arc::new(move |ctx| Box::pin(run(ctx))), wake: Notify::new() }));
        if let Ok(mut status) = self.0.status.lock() {
            status.insert(name, JobStatus {
                name,
                state: JobState::Idle,
                runs: 0,
                failures: 0,
                last_started_ms: None,
                last_finished_ms: None,
                last_error: None,
                every_ms: schedule.every.map(|d| d.as_millis()),
            });
        }
    }

    /// Asks a job to run as soon as a slot is free. A job triggered while it runs runs once more
    /// afterwards.
    pub fn trigger(&self, name: &str) -> Result<()> {
        if self.0.stopping.load(Ordering::SeqCst) {
            return Err(Error::Conflict);
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
        let names: Vec<&'static str> = self.0.jobs.lock().map(|j| j.iter().map(|j| j.name).collect()).unwrap_or_default();
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

    pub(crate) fn is_empty(&self) -> bool {
        self.0.jobs.lock().map(|j| j.is_empty()).unwrap_or(true)
    }

    /// Starts the schedulers on the current Tokio runtime, once; `ctx` builds the context of each
    /// run from the job name. `App` does this when it builds its router.
    pub fn start(&self, ctx: impl Fn(&'static str) -> Ctx + Send + Sync + 'static) {
        let ctx: Arc<dyn Fn(&'static str) -> Ctx + Send + Sync> = Arc::new(ctx);
        if self.0.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let jobs: Vec<Arc<Job>> = self.0.jobs.lock().map(|j| j.clone()).unwrap_or_default();
        for job in jobs {
            let me = self.clone();
            let ctx = ctx.clone();
            tokio::spawn(async move { me.schedule(job, ctx).await });
        }
    }

    async fn schedule(self, job: Arc<Job>, ctx: Arc<dyn Fn(&'static str) -> Ctx + Send + Sync>) {
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
            let result = (job.run)(ctx.clone()).await;
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
