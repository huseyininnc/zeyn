//! DevTools of debug builds: what each request did. For every request the server keeps which
//! screen and action ran, how long the screen took to build and to render, each SQL statement
//! with its time, and the effects and topics the action returned. The last [`KEEP`] requests are
//! served as JSON at `GET /_zeyn/devtools`, and Alt+Shift+D opens a panel that lists them
//! (`assets/devtools.js`).
//!
//! The whole module exists only under `cfg(debug_assertions)`, so release builds carry no code,
//! no route and no memory for it. It records SQL text without its arguments, names of screens,
//! actions, effects and topics, and times; never values, so masked data stays out of it.
//! `App::serve` turns it on; tests and `App::router` leave it off unless they call
//! [`devtools`].

use super::{AppState, asset};
use axum::Router;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value as Json, json};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How many requests the log keeps; older ones drop out.
pub(crate) const KEEP: usize = 50;

/// How many statements one request keeps; the rest are only counted.
#[cfg(any(feature = "postgres", feature = "oracle"))]
const QUERIES: usize = 100;

/// How much of a statement's text is kept.
#[cfg(any(feature = "postgres", feature = "oracle"))]
const SQL_CHARS: usize = 500;

const JS: &str = include_str!("../../assets/devtools.js");
const CSS: &str = include_str!("../../assets/devtools.css");

static ON: AtomicBool = AtomicBool::new(false);
static LOG: Mutex<VecDeque<Json>> = Mutex::new(VecDeque::new());

tokio::task_local! {
    static TRACE: Arc<Mutex<Trace>>;
}

/// Turns the DevTools of debug builds on or off for the whole process. `App::serve` turns them
/// on; an app that serves `App::router` itself, or a test, calls this. Release builds have no
/// DevTools and no such function.
///
/// ```
/// zeyn::server::devtools(true);
/// ```
pub fn devtools(on: bool) {
    ON.store(on, Ordering::Relaxed);
    if !on && let Ok(mut log) = LOG.lock() {
        log.clear();
    }
}

/// Whether the DevTools record requests.
pub(crate) fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// One SQL statement of a request.
struct Query {
    sql: String,
    ms: f64,
    rows: Option<u64>,
}

/// What one request did.
#[derive(Default)]
pub(crate) struct Trace {
    kind: Option<String>,
    screen: Option<&'static str>,
    action: Option<&'static str>,
    ok: Option<bool>,
    error: Option<&'static str>,
    build: Option<Duration>,
    render: Option<Duration>,
    queries: Vec<Query>,
    query_count: usize,
    query_time: Duration,
    effects: Vec<String>,
    topics: Vec<String>,
}

/// Runs `f` on the trace of the current request, when one is being recorded.
fn with(f: impl FnOnce(&mut Trace)) {
    let _ = TRACE.try_with(|t| {
        if let Ok(mut t) = t.lock() {
            f(&mut t);
        }
    });
}

/// Records the call the request made: its kind, screen, action and outcome.
pub(crate) fn call(
    kind: super::CallKind,
    screen: &'static str,
    action: Option<&'static str>,
    ok: bool,
    error: Option<&'static str>,
) {
    with(|t| {
        t.kind = serde_json::to_value(kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string));
        t.screen = Some(screen);
        t.action = action.or(t.action);
        t.ok = Some(ok);
        t.error = error;
    });
}

/// Records how long building the screen took.
pub(crate) fn build(took: Duration) {
    with(|t| t.build = Some(t.build.unwrap_or_default() + took));
}

/// Records how long rendering the screen took.
pub(crate) fn render(took: Duration) {
    with(|t| t.render = Some(t.render.unwrap_or_default() + took));
}

/// Records one SQL statement: its text (without arguments), time and affected rows.
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub(crate) fn query(sql: &str, took: Duration, rows: Option<u64>) {
    with(|t| {
        t.query_count += 1;
        t.query_time += took;
        if t.queries.len() < QUERIES {
            t.queries.push(Query {
                sql: sql.chars().take(SQL_CHARS).collect(),
                ms: ms(took),
                rows,
            });
        }
    });
}

/// Records the effects an action returned, and the topics of its refresh effects.
pub(crate) fn effects(effects: &[crate::screen::Effect]) {
    with(|t| {
        for e in effects {
            let kind = serde_json::to_value(e)
                .ok()
                .and_then(|v| v["effect"].as_str().map(str::to_string))
                .unwrap_or_else(|| "effect".into());
            t.effects.push(kind);
            if let crate::screen::Effect::Refresh { topics } = e {
                t.topics
                    .extend(topics.iter().map(|s| s.to_string()));
            }
        }
    });
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}

/// Whether a request is worth a line: runtime files and the DevTools' own requests are not.
fn recorded(path: &str) -> bool {
    !(path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".svg")
        || path.ends_with(".woff2")
        || path.ends_with(".ico")
        || path == "/_zeyn/devtools"
        || path.starts_with("/_zeyn/dev/"))
}

/// Middleware: records the request when the DevTools are on.
pub(crate) async fn layer(req: Request, next: Next) -> Response {
    if !on() || !recorded(req.uri().path()) {
        return next.run(req).await;
    }
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let started = Instant::now();
    let trace = Arc::new(Mutex::new(Trace::default()));
    let response = TRACE
        .scope(trace.clone(), next.run(req))
        .await;
    let took = started.elapsed();
    let Ok(t) = trace.lock() else {
        return response;
    };
    let line = json!({
        "at": crate::time::DateTime::now().to_string(),
        "method": method,
        "path": path,
        "status": response.status().as_u16(),
        "ms": ms(took),
        "kind": t.kind,
        "screen": t.screen,
        "action": t.action,
        "ok": t.ok,
        "error": t.error,
        "build_ms": t.build.map(ms),
        "render_ms": t.render.map(ms),
        "query_count": t.query_count,
        "query_ms": ms(t.query_time),
        "queries": t.queries.iter().map(|q| json!({ "sql": q.sql, "ms": q.ms, "rows": q.rows })).collect::<Vec<_>>(),
        "effects": t.effects,
        "topics": t.topics,
    });
    if let Ok(mut log) = LOG.lock() {
        if log.len() == KEEP {
            log.pop_back();
        }
        log.push_front(line);
    }
    response
}

/// `GET /_zeyn/devtools`: the recorded requests, newest first. Not found while the DevTools
/// are off.
async fn list() -> Response {
    if !on() {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    let log: Vec<Json> = LOG
        .lock()
        .map(|l| l.iter().cloned().collect())
        .unwrap_or_default();
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        axum::Json(json!({ "requests": log })),
    )
        .into_response()
}

/// The DevTools endpoint and their script and stylesheet.
pub(crate) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/devtools", get(list))
        .route(
            "/_zeyn/devtools.js",
            get(|| async { asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/devtools.css",
            get(|| async { asset(CSS, "text/css; charset=utf-8") }),
        )
}

/// The `<head>` script of the DevTools, while they are on.
pub(crate) fn head(prefix: &str) -> String {
    match on() {
        true => format!(
            r#"<script type="module" src="{}/_zeyn/devtools.js{}"></script>"#,
            crate::render::escape(prefix),
            super::ops::asset_version()
        ),
        false => String::new(),
    }
}
