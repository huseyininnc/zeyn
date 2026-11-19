//! Metronome (feature `metronome`): cron with seconds, time zones and daylight saving rules,
//! calendars, misfire policies, pause and resume, run now, timeouts, priorities, chains, exactly
//! one run per tick across schedulers, run history and the operations screen. Time comes from a
//! `FakeClock`; no test waits for a tick in real time.
//!
//! The SQL store runs the cluster test against real databases when `ZEYN_TEST_PG` (feature
//! `postgres`) or `ZEYN_TEST_ORACLE` (feature `oracle`) is set, as in `tests/jobs_store.rs`:
//!
//! ```text
//! docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55437:5432 postgres:17
//! ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55437/postgres cargo test -p zeyn --features metronome,postgres --test metronome
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeyn::metronome::{Calendar, Cron, FakeClock, Misfire, Resume, Trigger, Weekday};
use zeyn::prelude::*;
use zeyn::testing::TestApp;
use zeyn::{JobStore, Jobs, MemoryJobStore, Schedule};

const HOUR: i64 = 3_600_000;

fn at(text: &str) -> i64 {
    DateTime::parse(text)
        .unwrap_or_else(|| panic!("{text}"))
        .unix_ms()
}

fn zone(name: &str) -> TimeZone {
    TimeZone::named(name).unwrap()
}

/// The ticks of `expr` in `zone` after `from`, up to `count`.
fn ticks(expr: &str, zone_name: &str, from: &str, count: usize) -> Vec<String> {
    let cron = Cron::parse(expr).unwrap();
    let mut t = at(from);
    let mut out = Vec::new();
    for _ in 0..count {
        t = cron
            .next_after(t, zone(zone_name))
            .unwrap();
        out.push(utc(t));
    }
    out
}

fn utc(ms: i64) -> String {
    let local = DateTime::from_unix_ms(ms).local(TimeZone::UTC);
    format!("{}T{}Z", local.date, local.time)
}

#[test]
fn cron_reads_the_quartz_dialect_and_explains_mistakes() {
    for ok in [
        "0 0 12 * * ?",
        "0 15 10 ? * MON-FRI",
        "0 0/5 14,18 * * ?",
        "0 15 10 L * ?",
        "0 15 10 L-2 * ?",
        "0 15 10 15W * ?",
        "0 15 10 LW * ?",
        "0 15 10 ? * 6L",
        "0 15 10 ? * 6#3",
        "0 15 10 ? * MON#1 2026-2030",
        "*/10 * 22-2 * jan,DEC ?",
    ] {
        assert!(Cron::parse(ok).is_ok(), "{ok}");
    }
    let five = Cron::parse("30 7 * * 1-5").unwrap_err();
    assert!(five.contains("write `0 30 7 * * 1-5`"), "{five}");
    let both = Cron::parse("0 0 8 1 * MON").unwrap_err();
    assert!(both.contains("put `?` in one of them"), "{both}");
    let sunday = Cron::parse("0 0 8 ? * 0").unwrap_err();
    assert!(sunday.contains("1 is Sunday"), "{sunday}");
    for bad in [
        "60 * * * * *",
        "0 0 24 * * *",
        "0 0 0 32 * ?",
        "0 0 0 ? * MON#6",
        "0 */0 * * * *",
        "0 0 0 * FOO ?",
        "0 0 0 ? * * 1969",
        "a b c d e f",
    ] {
        assert!(Cron::parse(bad).is_err(), "{bad}");
    }
    assert_eq!(
        ticks("0 15 10 ? * 6L", "UTC", "2026-01-01T00:00:00Z", 2),
        ["2026-01-30T10:15:00Z", "2026-02-27T10:15:00Z"]
    );
    assert_eq!(
        ticks("0 0 9 ? * MON#1", "UTC", "2026-03-01T00:00:00Z", 2),
        ["2026-03-02T09:00:00Z", "2026-04-06T09:00:00Z"]
    );
    assert_eq!(
        ticks("0 0 9 1W * ?", "UTC", "2026-08-01T00:00:00Z", 1),
        ["2026-08-03T09:00:00Z"],
        "August 1 2026 is a Saturday; the nearest weekday in the month is Monday the 3rd"
    );
    assert_eq!(
        ticks("0 0 0 L * ? 2028", "UTC", "2026-01-01T00:00:00Z", 2),
        ["2028-01-31T00:00:00Z", "2028-02-29T00:00:00Z"]
    );
    assert!(
        Cron::parse("0 0 0 ? * * 2020")
            .unwrap()
            .next_after(at("2026-01-01T00:00:00Z"), TimeZone::UTC)
            .is_none()
    );
}

#[test]
fn a_tick_in_a_spring_gap_runs_once_at_the_end_of_the_gap() {
    assert_eq!(
        ticks("0 30 2 * * *", "Europe/Berlin", "2026-03-28T00:00:00Z", 3),
        [
            "2026-03-28T01:30:00Z",
            "2026-03-29T01:00:00Z",
            "2026-03-30T00:30:00Z"
        ],
        "02:30 does not exist on March 29; it runs at 03:00 CEST"
    );
    assert_eq!(
        ticks("0 0 * * * *", "Europe/Berlin", "2026-03-29T00:30:00Z", 3),
        [
            "2026-03-29T01:00:00Z",
            "2026-03-29T02:00:00Z",
            "2026-03-29T03:00:00Z"
        ],
        "the hourly 02:00 tick falls in the gap and runs once with 03:00"
    );
    assert_eq!(
        ticks(
            "0 30 2 * * *",
            "America/New_York",
            "2026-03-08T00:00:00Z",
            1
        ),
        ["2026-03-08T07:00:00Z"]
    );
    assert_eq!(
        ticks(
            "0 15 2 * * *",
            "Australia/Lord_Howe",
            "2026-10-03T12:00:00Z",
            2
        ),
        ["2026-10-03T15:30:00Z", "2026-10-04T15:15:00Z"],
        "Lord Howe moves 30 minutes: 02:15 runs at 02:30 local, the end of the gap"
    );
    assert_eq!(
        ticks("0 0 12 * * *", "Pacific/Apia", "2011-12-29T00:00:00Z", 3),
        [
            "2011-12-29T22:00:00Z",
            "2011-12-30T10:00:00Z",
            "2011-12-30T22:00:00Z"
        ],
        "Samoa skipped December 30 2011; its noon runs at the first instant of the 31st"
    );
}

#[test]
fn a_fixed_hour_runs_once_in_a_fold_and_a_step_follows_the_real_clock() {
    assert_eq!(
        ticks("0 30 2 * * *", "Europe/Berlin", "2026-10-24T12:00:00Z", 2),
        ["2026-10-25T00:30:00Z", "2026-10-26T01:30:00Z"],
        "02:30 happens twice on October 25 and runs at the first"
    );
    assert_eq!(
        ticks("0 0 */1 * * *", "Europe/Berlin", "2026-10-24T23:30:00Z", 4),
        [
            "2026-10-25T00:00:00Z",
            "2026-10-25T01:00:00Z",
            "2026-10-25T02:00:00Z",
            "2026-10-25T03:00:00Z"
        ],
        "an hourly step runs in both passes of the repeated hour"
    );
    assert_eq!(
        ticks(
            "0 30 1 * * *",
            "America/New_York",
            "2026-11-01T00:00:00Z",
            2
        ),
        ["2026-11-01T05:30:00Z", "2026-11-02T06:30:00Z"]
    );
    assert_eq!(
        ticks("0 0 7 * * *", "Europe/Istanbul", "2026-06-01T00:00:00Z", 1),
        ["2026-06-01T04:00:00Z"],
        "Istanbul stays at UTC+3 all year"
    );
}

#[test]
fn calendars_exclude_days_weekends_dates_and_ranges_across_midnight() {
    let c = Calendar::days(["2026-04-23"])
        .weekly_off([Weekday::Sunday])
        .annual([(5, 1)])
        .and(Calendar::default().ranges([("23:00", "01:00")]));
    let ist = zone("Europe/Istanbul");
    let excluded = |t: &str| c.excludes(DateTime::parse(t).unwrap(), ist);
    assert!(excluded("2026-04-23T09:00:00Z"));
    assert!(excluded("2026-04-26T09:00:00Z"), "a Sunday");
    assert!(excluded("2027-05-01T09:00:00Z"), "every May 1");
    assert!(excluded("2026-04-24T20:30:00Z"), "23:30 in Istanbul");
    assert!(excluded("2026-04-24T21:30:00Z"), "00:30 in Istanbul");
    assert!(!excluded("2026-04-24T09:00:00Z"));
    assert!(
        Trigger::cron("0 0 9 * * *")
            .time_zone("Mars/Olympus")
            .problem()
            .unwrap()
            .contains("IANA")
    );
    let jobs = Jobs::default();
    jobs.add_calendar("bad", Calendar::days(["2026-13-01"]));
    assert!(jobs.problems()[0].contains("`2026-13-01` is not a date"));
}

/// What a run of `report` saw: payload, trigger id, scheduled time and user.
type Seen = Arc<Mutex<Vec<(String, Option<String>, Option<i64>, Option<String>)>>>;

/// Jobs on a store and a fake clock, with a queued job `report` that records what it saw.
struct Rig {
    jobs: Jobs,
    store: MemoryJobStore,
    clock: FakeClock,
    seen: Seen,
}

fn rig(store: &MemoryJobStore, clock: &FakeClock) -> Rig {
    let jobs = Jobs::with_limit(4);
    jobs.set_clock(clock.clone());
    jobs.set_store(store.clone()).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    jobs.add_queued(
        "report",
        Schedule::manual(),
        move |ctx: Ctx, payload: String| {
            let s = s.clone();
            async move {
                s.lock().unwrap().push((
                    payload,
                    ctx.trigger_id().map(str::to_string),
                    ctx.scheduled_at().map(|t| t.unix_ms()),
                    ctx.identity().user.clone(),
                ));
                Ok(())
            }
        },
    );
    Rig {
        jobs,
        store: store.clone(),
        clock: clock.clone(),
        seen,
    }
}

/// The scheduled times of every queued run, oldest first, taking them out of the store.
async fn queued(store: &MemoryJobStore, now: i64) -> Vec<i64> {
    let mut out: Vec<i64> = store
        .claim(
            "test",
            &["report", "other"],
            now,
            1_000_000_000_000,
            100_000,
        )
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.scheduled_ms.unwrap_or(-1))
        .collect();
    out.sort_unstable();
    out
}

async fn wait_until(mut done: impl FnMut() -> bool) {
    for _ in 0..500 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out");
}

async fn downtime(policy: Misfire) -> (Vec<i64>, Vec<zeyn::metronome::RunRow>) {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_trigger(
        "report",
        Trigger::cron("0 0 * * * *")
            .id("hourly")
            .misfire(policy),
    );
    assert_eq!(
        r.jobs.run_due().await.unwrap(),
        0,
        "the first pass only stores the trigger"
    );
    r.clock
        .advance(Duration::from_secs(3600));
    assert_eq!(r.jobs.run_due().await.unwrap(), 1, "on time");
    assert_eq!(
        queued(&r.store, r.clock.now_ms()).await,
        [at("2026-01-05T01:00:00Z")]
    );
    r.clock
        .advance(Duration::from_secs(10 * 3600));
    r.jobs.run_due().await.unwrap();
    let runs = queued(&r.store, r.clock.now_ms()).await;
    let history = r
        .jobs
        .history(None, 0, 10)
        .await
        .unwrap();
    (runs, history)
}

use zeyn::metronome::Clock;

#[tokio::test]
async fn misfire_policies_after_a_downtime_of_nine_ticks() {
    let base = at("2026-01-05T00:00:00Z");
    let (runs, history) = downtime(Misfire::RunOnce).await;
    assert_eq!(
        runs,
        [base + 10 * HOUR, base + 11 * HOUR],
        "one run for the missed ticks, then the on time tick"
    );
    assert!(history.is_empty());

    let (runs, history) = downtime(Misfire::RunAll { max: 3 }).await;
    assert_eq!(
        runs,
        [
            base + 2 * HOUR,
            base + 3 * HOUR,
            base + 4 * HOUR,
            base + 11 * HOUR
        ]
    );
    assert_eq!(history.len(), 1);
    assert_eq!(
        (history[0].outcome.as_str(), history[0].attempt),
        ("skipped", 6)
    );

    let (runs, history) = downtime(Misfire::Skip).await;
    assert_eq!(runs, [base + 11 * HOUR]);
    assert_eq!(
        (history[0].outcome.as_str(), history[0].attempt),
        ("skipped", 9)
    );
    assert_eq!(history[0].trigger_id.as_deref(), Some("hourly"));
}

#[tokio::test]
async fn a_year_of_downtime_on_a_trigger_of_every_second_stays_bounded() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_trigger(
        "report",
        Trigger::cron("* * * * * *").misfire(Misfire::RunAll { max: 1_000 }),
    );
    r.jobs.run_due().await.unwrap();
    r.clock
        .advance(Duration::from_secs(365 * 86_400));
    let started = std::time::Instant::now();
    let queued_now = r.jobs.run_due().await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        queued_now,
        zeyn::metronome::MAX_CATCH_UP as usize,
        "capped at MAX_CATCH_UP"
    );
    assert_eq!(store.len(), 100);
    let next = r.jobs.triggers("report")[0].next_ms[0];
    assert!(next > r.clock.now_ms(), "the trigger jumped to the future");
    assert_eq!(
        r.jobs.run_due().await.unwrap(),
        0,
        "nothing more in the same second"
    );
}

#[tokio::test]
async fn calendar_days_are_neither_run_nor_counted_as_misfires() {
    let clock = FakeClock::at(at("2026-01-04T06:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_calendar(
        "workdays",
        Calendar::days(["2026-01-07"]).weekly_off([Weekday::Saturday, Weekday::Sunday]),
    );
    r.jobs.add_trigger(
        "report",
        Trigger::cron("0 0 9 * * *")
            .time_zone("Europe/Istanbul")
            .calendar("workdays")
            .misfire(Misfire::Skip),
    );
    r.jobs.run_due().await.unwrap();
    let mut days = Vec::new();
    for _ in 0..7 {
        r.clock
            .advance(Duration::from_secs(86_400));
        r.jobs.run_due().await.unwrap();
        days.extend(queued(&r.store, r.clock.now_ms()).await);
    }
    assert_eq!(
        days.iter()
            .map(|d| utc(*d))
            .collect::<Vec<_>>(),
        [
            "2026-01-05T06:00:00Z",
            "2026-01-06T06:00:00Z",
            "2026-01-08T06:00:00Z",
            "2026-01-09T06:00:00Z"
        ]
    );
    assert!(
        r.jobs
            .history(None, 0, 10)
            .await
            .unwrap()
            .is_empty(),
        "excluded days are no misfires"
    );
}

#[tokio::test]
async fn two_schedulers_on_one_store_queue_each_of_a_thousand_ticks_once() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let a = rig(&store, &clock);
    let b = rig(&store, &clock);
    for r in [&a, &b] {
        r.jobs
            .add_trigger("report", Trigger::cron("* * * * * *").id("every_second"));
    }
    a.jobs.run_due().await.unwrap();
    b.jobs.run_due().await.unwrap();
    for _ in 0..1_000 {
        clock.advance(Duration::from_secs(1));
        let (x, y) = tokio::join!(a.jobs.run_due(), b.jobs.run_due());
        x.unwrap();
        y.unwrap();
    }
    let runs = queued(&store, clock.now_ms()).await;
    let distinct: HashSet<i64> = runs.iter().copied().collect();
    assert_eq!(runs.len(), 1_000, "no tick twice, none lost");
    assert_eq!(distinct.len(), 1_000);
}

#[tokio::test]
async fn several_triggers_carry_their_payload_priority_and_identity() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_trigger(
        "report",
        Trigger::cron("0 0 1 * * *")
            .id("low")
            .payload("low"),
    );
    r.jobs.add_trigger(
        "report",
        Trigger::cron("0 0 1 * * *")
            .id("high")
            .payload("high")
            .priority(10),
    );
    r.jobs.run_due().await.unwrap();
    clock.advance(Duration::from_secs(3600));
    assert_eq!(r.jobs.run_due().await.unwrap(), 2);
    let first = store
        .claim("w", &["report"], clock.now_ms(), 60_000, 1)
        .await
        .unwrap();
    assert_eq!(first[0].payload, "high", "the higher priority goes first");
    let aged_low = zeyn::NewJob {
        name: "report".into(),
        payload: "old".into(),
        run_at_ms: clock.now_ms() - 301_000,
        max_attempts: 1,
        ..Default::default()
    };
    let fresh_high = zeyn::NewJob {
        payload: "fresh".into(),
        run_at_ms: clock.now_ms(),
        priority: 20,
        ..aged_low.clone()
    };
    store.enqueue(fresh_high).await.unwrap();
    store.enqueue(aged_low).await.unwrap();
    let next = store
        .claim("w", &["report"], clock.now_ms(), 60_000, 1)
        .await
        .unwrap();
    assert_eq!(
        next[0].payload, "old",
        "a run waiting over five minutes is not starved"
    );
}

#[tokio::test]
async fn runs_know_their_tick_and_trigger_and_run_as_the_job_not_the_caller() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_trigger(
        "report",
        Trigger::cron("0 0 1 * * *")
            .id("nightly")
            .payload("42"),
    );
    r.jobs.start(|name| {
        Ctx::for_tests(
            Identity::user(format!("job:{name}")),
            zeyn::Services::default(),
        )
    });
    r.jobs.run_due().await.unwrap();
    clock.advance(Duration::from_secs(3600));
    r.jobs.run_due().await.unwrap();
    let alice = Ctx::for_tests(Identity::user("alice"), zeyn::Services::default());
    r.jobs
        .run_now(&alice, "report", "7")
        .await
        .unwrap();
    wait_until(|| r.seen.lock().unwrap().len() == 2).await;
    let seen = r.seen.lock().unwrap().clone();
    let nightly = seen
        .iter()
        .find(|s| s.0 == "42")
        .unwrap();
    assert_eq!(nightly.1.as_deref(), Some("nightly"));
    assert_eq!(nightly.2, Some(at("2026-01-05T01:00:00Z")));
    let manual = seen
        .iter()
        .find(|s| s.0 == "7")
        .unwrap();
    assert_eq!(manual.1.as_deref(), Some("manual"));
    assert_eq!(
        manual.3.as_deref(),
        Some("job:report"),
        "never the caller's identity"
    );
    wait_until(|| store.is_empty()).await;
    let history = r
        .jobs
        .history(Some("report"), 0, 10)
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    let by: HashMap<_, _> = history
        .iter()
        .map(|h| {
            (
                h.trigger_id.clone().unwrap_or_default(),
                h.started_by.clone(),
            )
        })
        .collect();
    assert_eq!(by["manual"].as_deref(), Some("alice"));
    assert!(
        history
            .iter()
            .all(|h| h.outcome == "succeeded")
    );
    let json = serde_json::to_string(&history).unwrap();
    assert!(
        !json.contains("\"42\"") && !json.contains("\"7\""),
        "no payload in history: {json}"
    );
    r.jobs
        .shutdown(Duration::from_secs(1))
        .await;
}

#[tokio::test]
async fn pause_and_resume_jobs_and_triggers_across_schedulers() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let a = rig(&store, &clock);
    let b = rig(&store, &clock);
    for r in [&a, &b] {
        r.jobs
            .add_trigger("report", Trigger::cron("0 0 * * * *").id("hourly"));
        r.jobs
            .add_trigger("report", Trigger::cron("0 30 * * * *").id("half"));
        r.jobs.run_due().await.unwrap();
    }
    a.jobs.pause("report").await.unwrap();
    assert!(a.jobs.is_paused("report"));
    assert!(!b.jobs.is_paused("report"), "b reads pauses on its refresh");
    clock.advance(zeyn::metronome::TRIGGER_REFRESH);
    b.jobs.run_due().await.unwrap();
    assert!(
        b.jobs.is_paused("report"),
        "seen within the refresh interval"
    );
    clock.advance(Duration::from_secs(3 * 3600));
    assert_eq!(a.jobs.run_due().await.unwrap(), 0);
    assert_eq!(b.jobs.run_due().await.unwrap(), 0);
    let ctx = Ctx::for_tests(Identity::user("alice"), zeyn::Services::default());
    assert_eq!(
        a.jobs.run_now(&ctx, "report", "").await,
        Err(Error::Conflict)
    );

    a.jobs
        .resume("report", Resume::Skip)
        .await
        .unwrap();
    assert_eq!(
        a.jobs.run_due().await.unwrap(),
        0,
        "the paused hours are dropped"
    );
    a.jobs.pause("report").await.unwrap();
    clock.advance(Duration::from_secs(2 * 3600 + 900));
    a.jobs
        .resume("report", Resume::Misfire)
        .await
        .unwrap();
    assert_eq!(
        a.jobs.run_due().await.unwrap(),
        2,
        "one catch up run per trigger"
    );
    queued(&store, clock.now_ms()).await;

    a.jobs
        .pause_trigger("report", "half")
        .await
        .unwrap();
    clock.advance(Duration::from_secs(3600));
    assert_eq!(
        a.jobs.run_due().await.unwrap(),
        1,
        "only the hourly trigger runs"
    );
    assert!(
        a.jobs
            .pause_trigger("report", "missing")
            .await
            .is_err()
    );
    a.jobs
        .resume_trigger("report", "half", Resume::Skip)
        .await
        .unwrap();
    let status = a.jobs.status();
    let triggers = &status[0].triggers;
    assert_eq!(triggers.len(), 2);
    assert!(
        triggers
            .iter()
            .all(|t| !t.paused && t.next_ms.len() == 3)
    );
}

#[tokio::test]
async fn a_paused_job_keeps_its_queued_runs_until_it_is_resumed() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs
        .add_trigger("report", Trigger::cron("0 0 * * * *"));
    r.jobs.pause("report").await.unwrap();
    r.jobs
        .start(|_| Ctx::for_tests(Identity::user("job"), zeyn::Services::default()));
    r.jobs
        .enqueue("report", "waiting")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        r.seen.lock().unwrap().is_empty(),
        "no worker claims a paused job"
    );
    r.jobs
        .resume("report", Resume::Skip)
        .await
        .unwrap();
    r.jobs
        .enqueue("report", "wake")
        .await
        .unwrap();
    wait_until(|| r.seen.lock().unwrap().len() == 2).await;
    r.jobs
        .shutdown(Duration::from_secs(1))
        .await;
}

#[tokio::test]
async fn a_run_past_its_timeout_fails_with_the_code_timeout_and_chains_run_on_success_only() {
    let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
    let store = MemoryJobStore::default();
    let r = rig(&store, &clock);
    r.jobs.add_queued(
        "slow",
        Schedule::manual(),
        |_ctx: Ctx, _payload: String| async {
            std::future::pending::<()>().await;
            Ok(())
        },
    );
    r.jobs
        .add_queued("other", Schedule::manual(), |_ctx: Ctx, _p: String| async {
            Err(Error::Internal("fails".into()))
        });
    r.jobs.add_trigger(
        "slow",
        Trigger::cron("0 0 * * * *")
            .id("hourly")
            .timeout(Duration::from_millis(20)),
    );
    r.jobs.add_chain("other", "report");
    r.jobs.add_chain("slow", "report");
    r.jobs
        .start(|_| Ctx::for_tests(Identity::user("job"), zeyn::Services::default()));
    r.jobs.run_due().await.unwrap();
    clock.advance(Duration::from_secs(3600));
    r.jobs.run_due().await.unwrap();
    r.jobs
        .enqueue("other", "x")
        .await
        .unwrap();
    let history = loop {
        let h = r
            .jobs
            .history(None, 0, 10)
            .await
            .unwrap();
        if h.len() >= 2 {
            break h;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    let slow = history
        .iter()
        .find(|h| h.job == "slow")
        .unwrap();
    assert_eq!(
        (slow.outcome.as_str(), slow.error_code.as_deref()),
        ("timeout", Some("timeout"))
    );
    assert!(
        r.seen.lock().unwrap().is_empty(),
        "no chain after a failure or a timeout"
    );

    let ctx = Ctx::for_tests(Identity::user("alice"), zeyn::Services::default());
    r.jobs.add_chain("report", "other");
    r.jobs
        .run_now(&ctx, "report", "chained")
        .await
        .unwrap();
    wait_until(|| r.seen.lock().unwrap().len() == 1).await;
    let runs = loop {
        let h = r
            .jobs
            .history(Some("other"), 0, 10)
            .await
            .unwrap();
        if h.len() >= 2 {
            break h;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert!(
        runs.iter()
            .any(|h| h.trigger_id.as_deref() == Some("chain")
                && h.started_by.as_deref() == Some("job:report")),
        "{runs:?}"
    );
    r.jobs
        .shutdown(Duration::from_secs(1))
        .await;
}

#[test]
fn chains_with_cycles_and_unknown_names_are_refused_at_startup() {
    let jobs = Jobs::default();
    for name in ["a", "b", "c"] {
        jobs.add(name, Schedule::manual(), |_ctx: Ctx| async { Ok(()) });
    }
    jobs.add_chain("a", "b");
    jobs.add_chain("b", "c");
    assert!(jobs.problems().is_empty(), "{:?}", jobs.problems());
    jobs.add_chain("c", "a");
    jobs.add_chain("c", "nope");
    let problems = jobs.problems().join("\n");
    assert!(problems.contains("is a cycle"), "{problems}");
    assert!(
        problems.contains("`nope`, which is not registered"),
        "{problems}"
    );
}

#[test]
fn bad_triggers_stop_the_app_with_messages_that_name_them() {
    let app = App::new("T")
        .job("report", Schedule::manual(), |_ctx: Ctx| async { Ok(()) })
        .trigger("report", Trigger::cron("0 30 7 * * MON-FRI").id("morning"))
        .trigger("report", Trigger::cron("0 30 7 * * MON-FRI").id("morning"))
        .trigger("report", Trigger::cron("30 7 * * *").id("five"))
        .trigger("missing", Trigger::cron("0 0 0 * * *").id("orphan"))
        .trigger(
            "report",
            Trigger::cron("0 0 0 * * *")
                .id("closed")
                .calendar("holidays"),
        )
        .trigger("report", Trigger::cron("0 0 0 30 FEB ?").id("never"))
        .trigger("report", Trigger::cron("0 0 0 * * *").id("Bad Id"));
    let problems = app.check().unwrap_err().join("\n");
    for expected in [
        "two triggers with the id `morning`",
        "trigger `five` of job `report`: `30 7 * * *` has five fields",
        "names the job `missing`, which is not registered",
        "names the calendar `holidays`, which is not registered",
        "trigger `never` of job `report` never runs",
        "`Bad Id` is not a trigger id",
    ] {
        assert!(
            problems.contains(expected),
            "missing `{expected}` in:\n{problems}"
        );
    }
    assert!(problems.contains("Z0508"));
}

#[test]
fn the_memory_history_is_a_ring_of_fixed_size() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let store = MemoryJobStore::default();
            let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
            let r = rig(&store, &clock);
            r.jobs
                .set_retention(zeyn::metronome::Retention::days(3_650));
            r.jobs.add_trigger(
                "report",
                Trigger::cron("0 0 * * * *").misfire(Misfire::Skip),
            );
            r.jobs.run_due().await.unwrap();
            for _ in 0..1_500 {
                clock.advance(Duration::from_secs(3 * 3600));
                r.jobs.run_due().await.unwrap();
                queued(&store, clock.now_ms()).await;
            }
            let all = r
                .jobs
                .history(None, 0, 2_000)
                .await
                .unwrap();
            assert_eq!(all.len(), 500, "pages stop at MAX_PAGE_SIZE");
            let deep = r
                .jobs
                .history(None, 900, 500)
                .await
                .unwrap();
            assert_eq!(deep.len(), 100, "1,000 rows kept of 1,500");
            assert_eq!(
                store
                    .prune_history(clock.now_ms() + 1, 10, 1_000)
                    .await
                    .unwrap(),
                1_000,
                "everything ended before now"
            );
        });
}

#[zeyn::screen(route = "/home", public)]
pub fn home(_: ()) -> Screen {
    Screen::new("Home")
}

fn ops_app() -> App {
    App::new("Ops")
        .job("report", Schedule::manual(), |_ctx: Ctx| async { Ok(()) })
        .trigger(
            "report",
            Trigger::cron("0 0 2 * * *")
                .id("nightly")
                .time_zone("Europe/Istanbul"),
        )
        .jobs_permission("zeyn.jobs.read")
        .feature(zeyn::metronome::ops())
        .feature(Feature::new("home").screen(home))
}

#[tokio::test]
async fn the_operations_screen_and_endpoints_check_each_permission_on_the_server() {
    let reader = TestApp::signed_in(
        ops_app(),
        Identity::user("rita").with_permissions(["zeyn.jobs.read"]),
    );
    let page = reader.get("/ops/jobs").await;
    assert_eq!(page.status, 200, "{}", page.body);
    assert!(page.body.contains("data-zeyn-topics") || page.body.contains("zeyn.jobs"));
    let jobs = reader
        .grid(
            "zeyn_jobs",
            serde_json::json!(null),
            "0.0.0",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(jobs.status, 200, "{}", jobs.body);
    assert!(jobs.body.contains("report"), "{}", jobs.body);
    let triggers = reader
        .grid(
            "zeyn_jobs",
            serde_json::json!(null),
            "1.0.0",
            serde_json::json!({}),
        )
        .await;
    assert!(
        triggers.body.contains("0 0 2 * * *")
            && triggers
                .body
                .contains("Europe/Istanbul"),
        "{}",
        triggers.body
    );
    let status = reader.get("/_zeyn/jobs").await;
    assert_eq!(status.status, 200);
    assert_eq!(status.json()["jobs"][0]["triggers"][0]["id"], "nightly");
    assert_eq!(
        reader
            .get("/_zeyn/jobs/runs")
            .await
            .status,
        200
    );

    let outsider = TestApp::signed_in(ops_app(), Identity::user("otto"));
    assert_eq!(outsider.get("/ops/jobs").await.status, 403);
    assert_eq!(
        outsider
            .get("/_zeyn/jobs/runs")
            .await
            .status,
        403
    );
    let grid = outsider
        .grid(
            "zeyn_jobs",
            serde_json::json!(null),
            "0.0.0",
            serde_json::json!({}),
        )
        .await;
    assert_eq!(grid.status, 403);
    assert!(
        !jobs.body.contains("Run now"),
        "no button without the run permission"
    );
    let run = |who: &TestApp| {
        let who = who.clone();
        async move {
            who.post(
                "/_zeyn/grid/run",
                serde_json::json!({ "screen": "zeyn_jobs", "node": "0.0.0", "key": "report", "item": "a0" }),
            )
            .await
            .status
        }
    };
    assert_eq!(
        run(&reader).await,
        403,
        "run now is refused on the server without zeyn.jobs.run"
    );
    let operator = TestApp::signed_in(
        ops_app(),
        Identity::user("omar").with_permissions(["zeyn.jobs.read", "zeyn.jobs.run"]),
    );
    let rows = operator
        .grid(
            "zeyn_jobs",
            serde_json::json!(null),
            "0.0.0",
            serde_json::json!({}),
        )
        .await;
    assert!(rows.body.contains("Run now"), "{}", rows.body);
    assert_eq!(run(&operator).await, 200);
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
async fn two_servers_on_a_sql_store_queue_each_tick_once_and_keep_pauses_and_history() {
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
        eprintln!("skipped: set ZEYN_TEST_PG or ZEYN_TEST_ORACLE to run the SQL scheduler tests");
    }
    for url in targets {
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        let prefix = format!("m{}_", std::process::id() % 100_000);
        let store = || {
            SqlJobStore::new(db.clone())
                .prefix(&prefix)
                .unwrap()
        };
        store().create_tables().await.unwrap();
        let clock = FakeClock::at(at("2026-01-05T00:00:00Z"));
        let servers: Vec<Jobs> = (0..2)
            .map(|_| {
                let jobs = Jobs::with_limit(4);
                jobs.set_clock(clock.clone());
                jobs.set_store(store()).unwrap();
                jobs.add_queued("report", Schedule::manual(), |_c: Ctx, _p: String| async {
                    Ok(())
                });
                jobs.add_trigger("report", Trigger::cron("* * * * * *").id("every_second"));
                jobs
            })
            .collect();
        for s in &servers {
            s.run_due().await.unwrap();
        }
        for _ in 0..1_000 {
            clock.advance(Duration::from_secs(1));
            let (x, y) = tokio::join!(servers[0].run_due(), servers[1].run_due());
            x.unwrap();
            y.unwrap();
        }
        let runs = store()
            .claim("test", &["report"], clock.now_ms(), 60_000, 2_000)
            .await
            .unwrap();
        let ticks: HashSet<Option<i64>> = runs
            .iter()
            .map(|r| r.scheduled_ms)
            .collect();
        assert_eq!(
            (runs.len(), ticks.len()),
            (1_000, 1_000),
            "each tick exactly once on {url}"
        );
        for run in runs.iter().take(3) {
            assert!(
                store()
                    .complete(run, "test", clock.now_ms())
                    .await
                    .unwrap()
            );
        }
        let history = servers[0]
            .history(Some("report"), 0, 10)
            .await
            .unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].trigger_id.as_deref(), Some("every_second"));
        servers[0]
            .pause("report")
            .await
            .unwrap();
        assert_eq!(store().paused_jobs().await.unwrap(), ["report"]);
        servers[0]
            .resume("report", Resume::Skip)
            .await
            .unwrap();
        assert!(
            store()
                .paused_jobs()
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store()
                .prune_history(clock.now_ms() + 1, 1, 1_000)
                .await
                .unwrap(),
            3
        );
    }
}
