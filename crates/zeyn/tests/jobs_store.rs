//! Durable jobs: the [`JobStore`] semantics on the in-memory store (claims, leases, retries,
//! schedules, capacity), `Jobs` over a shared store (payloads, restarts, two workers), and the
//! SQL store against real databases when `ZEYN_TEST_PG` (feature `postgres`) or
//! `ZEYN_TEST_ORACLE` (feature `oracle`) names one or more, as in `tests/postgres.rs`:
//!
//! ```text
//! docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
//! ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test jobs_store
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeyn::prelude::*;
use zeyn::{ClaimedJob, JobStore, Jobs, MemoryJobStore, NewJob, NextRun, Schedule};

const NAMES: &[&str] = &["mail", "report"];

fn new_job(name: &str, at: i64, max_attempts: u32) -> NewJob {
    NewJob {
        name: name.into(),
        payload: format!("{name}-payload"),
        run_at_ms: at,
        max_attempts,
        backoff_ms: 100,
    }
}

/// Every check of the store contract; the SQL stores run the same function.
async fn store_contract(store: &dyn JobStore) {
    let id = store
        .enqueue(new_job("mail", 1_000, 3))
        .await
        .unwrap();
    assert!(
        store
            .claim("a", NAMES, 999, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "not due yet"
    );
    assert!(
        store
            .claim("a", &["other"], 1_000, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "only named jobs"
    );

    let got = store
        .claim("a", NAMES, 1_000, 50, 10)
        .await
        .unwrap();
    assert_eq!(got.len(), 1);
    let first = got[0].clone();
    assert_eq!(
        (first.id.as_str(), first.payload.as_str(), first.attempts),
        (id.as_str(), "mail-payload", 1)
    );
    assert!(
        store
            .claim("b", NAMES, 1_020, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "a holds the lease"
    );
    assert!(
        store
            .heartbeat(&id, "a", 1_100)
            .await
            .unwrap(),
        "a renews"
    );
    assert!(
        !store
            .heartbeat(&id, "b", 1_100)
            .await
            .unwrap(),
        "b does not hold it"
    );
    assert!(
        store
            .claim("b", NAMES, 1_090, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "renewed lease holds"
    );

    let again = store
        .claim("b", NAMES, 1_101, 50, 10)
        .await
        .unwrap();
    assert_eq!(again.len(), 1, "an ended lease makes the run claimable");
    assert_eq!(again[0].attempts, 2, "the lost try counts");
    assert!(
        !store
            .complete(&first, "a", 1_102)
            .await
            .unwrap(),
        "a lost the run and cannot finish it"
    );

    assert!(
        store
            .fail(&again[0], "b", "internal", 1_200)
            .await
            .unwrap()
    );
    assert!(
        store
            .claim("b", NAMES, 1_200, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "retry waits for the backoff"
    );
    let third = store
        .claim("b", NAMES, 1_400, 50, 10)
        .await
        .unwrap();
    assert_eq!(third[0].attempts, 3, "second retry after 2 x 100 ms");
    assert!(
        store
            .fail(&third[0], "b", "internal", 1_500)
            .await
            .unwrap()
    );
    assert!(
        store
            .claim("b", NAMES, 999_999, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "no tries left: dead"
    );

    let ok = store
        .enqueue(new_job("report", 0, 1))
        .await
        .unwrap();
    let got = store
        .claim("a", NAMES, 10, 50, 10)
        .await
        .unwrap();
    assert_eq!(got[0].id, ok);
    assert!(
        store
            .complete(&got[0], "a", 11)
            .await
            .unwrap()
    );
    assert!(
        store
            .claim("a", NAMES, 999_999, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "done runs are gone"
    );

    store
        .schedule("report", 1_000, 5_000, 2, 10)
        .await
        .unwrap();
    store
        .schedule("report", 1_000, 9_000, 2, 10)
        .await
        .unwrap();
    assert!(
        store
            .claim("a", NAMES, 4_999, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "the stored next run is kept"
    );
    let tick = store
        .claim("a", NAMES, 5_000, 50, 10)
        .await
        .unwrap();
    assert_eq!(tick[0].every_ms, Some(1_000));
    assert!(
        store
            .complete(&tick[0], "a", 5_010)
            .await
            .unwrap()
    );
    assert!(
        store
            .claim("a", NAMES, 6_009, 50, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let tick = store
        .claim("a", NAMES, 6_010, 50, 10)
        .await
        .unwrap();
    assert_eq!(tick[0].attempts, 1, "a success resets the tries");
    assert!(
        store
            .fail(&tick[0], "a", "internal", 6_100)
            .await
            .unwrap()
    );
    let retry = store
        .claim("a", NAMES, 6_110, 50, 10)
        .await
        .unwrap();
    assert_eq!(retry[0].attempts, 2);
    assert!(
        store
            .fail(&retry[0], "a", "internal", 6_200)
            .await
            .unwrap()
    );
    assert!(
        store
            .claim("a", NAMES, 7_199, 50, 10)
            .await
            .unwrap()
            .is_empty(),
        "out of tries: next interval"
    );
    let next = store
        .claim("a", NAMES, 7_200, 50, 10)
        .await
        .unwrap();
    assert_eq!(next[0].attempts, 1, "a schedule never dies");
    assert!(
        store
            .complete(&next[0], "a", 7_200)
            .await
            .unwrap()
    );

    for i in 0..5 {
        store
            .enqueue(new_job("mail", 100 + i, 1))
            .await
            .unwrap();
    }
    let a = store
        .claim("a", NAMES, 200, 50, 3)
        .await
        .unwrap();
    let b = store
        .claim("b", NAMES, 200, 50, 3)
        .await
        .unwrap();
    assert_eq!(
        (a.len(), b.len()),
        (3, 2),
        "the limit holds and no run is taken twice"
    );
    assert!(
        a.iter()
            .all(|x| b.iter().all(|y| x.id != y.id))
    );
    for j in a.iter().chain(&b) {
        assert!(
            store
                .complete(j, if a.contains(j) { "a" } else { "b" }, 300)
                .await
                .unwrap()
        );
    }
}

#[tokio::test]
async fn the_memory_store_claims_with_leases_retries_with_backoff_and_keeps_schedules() {
    store_contract(&MemoryJobStore::default()).await;
}

#[test]
fn the_next_run_doubles_the_backoff_up_to_ten_minutes_and_schedules_never_die() {
    let job = |attempts, every_ms| ClaimedJob {
        id: "x".into(),
        name: "mail".into(),
        payload: String::new(),
        attempts,
        max_attempts: 30,
        backoff_ms: 1_000,
        every_ms,
    };
    assert_eq!(
        job(1, None).after_failure(0),
        NextRun::Queue {
            run_at_ms: 1_000,
            attempts: 1
        }
    );
    assert_eq!(
        job(3, None).after_failure(0),
        NextRun::Queue {
            run_at_ms: 4_000,
            attempts: 3
        }
    );
    assert_eq!(
        job(25, None).after_failure(0),
        NextRun::Queue {
            run_at_ms: 600_000,
            attempts: 25
        }
    );
    assert_eq!(job(30, None).after_failure(0), NextRun::Dead);
    assert_eq!(
        job(30, Some(60_000)).after_failure(5),
        NextRun::Queue {
            run_at_ms: 60_005,
            attempts: 0
        }
    );
    assert_eq!(job(1, None).after_success(0), NextRun::Done);
}

#[tokio::test]
async fn the_memory_store_is_bounded_and_drops_the_oldest_failed_run_first() {
    let store = MemoryJobStore::with_capacity(2);
    store
        .enqueue(new_job("mail", 0, 1))
        .await
        .unwrap();
    let dead = store
        .claim("a", NAMES, 0, 10, 1)
        .await
        .unwrap()
        .remove(0);
    store
        .fail(&dead, "a", "internal", 1)
        .await
        .unwrap();
    assert_eq!(store.failed(&dead.id), Some("internal"));
    store
        .enqueue(new_job("mail", 5, 1))
        .await
        .unwrap();
    store
        .enqueue(new_job("mail", 6, 1))
        .await
        .unwrap();
    assert_eq!(store.len(), 2);
    assert_eq!(store.failed(&dead.id), None, "the failed run made room");
    assert!(
        store
            .enqueue(new_job("mail", 7, 1))
            .await
            .is_err(),
        "full of live runs"
    );
    let big = NewJob {
        payload: "x".repeat(zeyn::MAX_JOB_PAYLOAD + 1),
        ..new_job("mail", 0, 1)
    };
    assert!(
        MemoryJobStore::default()
            .enqueue(big)
            .await
            .is_err(),
        "payloads are capped"
    );
}

fn ctx() -> Ctx {
    Ctx::for_tests(Identity::user("job"), zeyn::Services::default())
}

async fn wait_until(mut done: impl FnMut() -> bool) {
    for _ in 0..600 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out");
}

fn recording(jobs: &Jobs, seen: &Arc<Mutex<HashMap<String, u32>>>, fail_first: bool) {
    let seen = seen.clone();
    jobs.add_queued(
        "send",
        Schedule::manual().retry(2, Duration::from_millis(5)),
        move |_ctx: Ctx, payload: String| {
            let seen = seen.clone();
            async move {
                let n = {
                    let mut s = seen.lock().unwrap();
                    let n = s.entry(payload).or_insert(0);
                    *n += 1;
                    *n
                };
                if fail_first && n == 1 {
                    Err(Error::Internal("first try fails".into()))
                } else {
                    Ok(())
                }
            }
        },
    );
}

#[tokio::test]
async fn queued_runs_carry_their_payload_retry_and_survive_a_restart_through_the_store() {
    let store = MemoryJobStore::default();
    let seen = Arc::new(Mutex::new(HashMap::new()));

    let before = Jobs::default();
    before.set_store(store.clone()).unwrap();
    recording(&before, &seen, true);
    assert!(
        before
            .enqueue("missing", "")
            .await
            .is_err(),
        "only registered jobs"
    );
    before
        .enqueue_after("send", "later", Duration::from_millis(2_000))
        .await
        .unwrap();
    before.start(|_| ctx());
    before
        .enqueue("send", "now")
        .await
        .unwrap();
    wait_until(|| seen.lock().unwrap().get("now") == Some(&2)).await;
    assert!(
        before
            .shutdown(Duration::from_secs(1))
            .await
    );
    assert!(
        !seen
            .lock()
            .unwrap()
            .contains_key("later"),
        "stopped before it was due"
    );
    assert_eq!(store.len(), 1, "the waiting run is still stored");

    let after = Jobs::default();
    after.set_store(store.clone()).unwrap();
    assert!(
        after
            .set_store(MemoryJobStore::default())
            .is_err(),
        "the store is set once"
    );
    recording(&after, &seen, true);
    after.start(|_| ctx());
    wait_until(|| seen.lock().unwrap().get("later") == Some(&2)).await;
    wait_until(|| store.is_empty()).await;
    let status = after
        .status()
        .into_iter()
        .find(|s| s.name == "send")
        .unwrap();
    assert_eq!((status.runs, status.failures), (2, 1));
    after
        .shutdown(Duration::from_secs(1))
        .await;
}

#[tokio::test]
async fn two_workers_on_one_store_never_run_the_same_run() {
    let store = MemoryJobStore::default();
    let seen = Arc::new(Mutex::new(HashMap::new()));
    let workers: Vec<Jobs> = (0..2)
        .map(|_| {
            let jobs = Jobs::with_limit(3);
            jobs.set_store(store.clone()).unwrap();
            recording(&jobs, &seen, false);
            jobs
        })
        .collect();
    for i in 0..60 {
        workers[i % 2]
            .enqueue("send", format!("run-{i}"))
            .await
            .unwrap();
    }
    for w in &workers {
        w.start(|_| ctx());
    }
    wait_until(|| seen.lock().unwrap().len() == 60).await;
    wait_until(|| store.is_empty()).await;
    assert!(
        seen.lock()
            .unwrap()
            .values()
            .all(|n| *n == 1),
        "each run ran exactly once"
    );
    for w in &workers {
        w.shutdown(Duration::from_secs(1)).await;
    }
}

#[tokio::test]
async fn a_durable_schedule_is_stored_once_and_triggers_go_through_the_store() {
    let store = MemoryJobStore::default();
    let ticks = Arc::new(Mutex::new(0u32));
    let jobs = Jobs::default();
    jobs.set_store(store.clone()).unwrap();
    let t = ticks.clone();
    jobs.add(
        "tick",
        Schedule::every(Duration::from_millis(30)),
        move |_ctx: Ctx| {
            let t = t.clone();
            async move {
                *t.lock().unwrap() += 1;
                Ok(())
            }
        },
    );
    jobs.start(|_| ctx());
    wait_until(|| *ticks.lock().unwrap() >= 2).await;
    assert_eq!(store.len(), 1, "one stored row per schedule");
    let before = *ticks.lock().unwrap();
    jobs.trigger("tick").unwrap();
    wait_until(|| *ticks.lock().unwrap() > before).await;
    jobs.shutdown(Duration::from_secs(1))
        .await;
}

#[cfg(any(feature = "postgres", feature = "oracle"))]
fn urls(var: &str) -> Vec<String> {
    std::env::var(var)
        .map(|v| {
            v.split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(any(feature = "postgres", feature = "oracle"))]
#[tokio::test]
async fn the_sql_store_keeps_the_contract_and_two_servers_share_one_queue() {
    use zeyn::SqlJobStore;
    use zeyn::sql::{Db, PoolLimits};
    let mut targets = Vec::new();
    if cfg!(feature = "postgres") {
        targets.extend(urls("ZEYN_TEST_PG"));
    }
    if cfg!(feature = "oracle") {
        targets.extend(urls("ZEYN_TEST_ORACLE"));
    }
    if targets.is_empty() {
        eprintln!("skipped: set ZEYN_TEST_PG or ZEYN_TEST_ORACLE to run the SQL job store tests");
    }
    for url in targets {
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        let prefix = format!("j{}_", std::process::id() % 100_000);
        let store = SqlJobStore::new(db.clone())
            .prefix(&prefix)
            .unwrap();
        store.create_tables().await.unwrap();
        store_contract(&store).await;

        let seen = Arc::new(Mutex::new(HashMap::new()));
        let servers: Vec<Jobs> = (0..2)
            .map(|_| {
                let jobs = Jobs::with_limit(4);
                jobs.set_store(
                    SqlJobStore::new(db.clone())
                        .prefix(&prefix)
                        .unwrap(),
                )
                .unwrap();
                recording(&jobs, &seen, false);
                jobs
            })
            .collect();
        for i in 0..40 {
            servers[i % 2]
                .enqueue("send", format!("run-{i}"))
                .await
                .unwrap();
        }
        for s in &servers {
            s.start(|_| ctx());
        }
        wait_until(|| seen.lock().unwrap().len() == 40).await;
        assert!(
            seen.lock()
                .unwrap()
                .values()
                .all(|n| *n == 1),
            "each run ran exactly once"
        );
        for s in &servers {
            s.shutdown(Duration::from_secs(5)).await;
        }
    }
}
