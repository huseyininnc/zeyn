//! Local prototype of the browser playground (design in `docs/playground.md`).
//!
//! Two origins on two ports: the editor (a code box, a Run button and a preview frame) and the
//! preview, which serves the rendered pages and the zeyn runtime assets, never the editor. Code
//! is compiled and run only inside the sandbox container, never in this process.
//!
//! ```text
//! docker build -f tools/playground/sandbox/Dockerfile -t zeyn-playground-sandbox .
//! cargo run -p zeyn-playground
//! open http://127.0.0.1:3120
//! ```
//!
//! `ZEYN_PLAYGROUND_EDITOR`, `ZEYN_PLAYGROUND_PREVIEW` and `ZEYN_PLAYGROUND_IMAGE` change the
//! two addresses and the sandbox image.

mod check;
mod runner;

use axum::Router;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Rendered pages kept by source hash.
const CACHE_PAGES: usize = 256;

/// Containers that compile at the same time.
const SLOTS: usize = 2;

/// Requests that may wait for a slot; more are turned away with 503.
const QUEUE: usize = 8;

/// Runs one address may start per window.
const RUNS_PER_WINDOW: u32 = 10;

/// The rate limit window.
const WINDOW: Duration = Duration::from_secs(60);

/// Addresses the rate limiter remembers; the oldest windows are dropped first.
const MAX_ADDRESSES: usize = 10_000;

/// The shared state of both origins.
struct Play {
    image: String,
    editor_origin: String,
    preview_origin: String,
    pages: Mutex<Pages>,
    slots: Semaphore,
    waiting: AtomicUsize,
    runs: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

/// A capped cache of rendered pages, oldest out first.
#[derive(Default)]
struct Pages {
    by_id: HashMap<String, Arc<str>>,
    order: VecDeque<String>,
}

impl Pages {
    fn get(&self, id: &str) -> Option<Arc<str>> {
        self.by_id.get(id).cloned()
    }

    fn put(&mut self, id: String, page: Arc<str>) {
        if self
            .by_id
            .insert(id.clone(), page)
            .is_none()
        {
            self.order.push_back(id);
        }
        while self.order.len() > CACHE_PAGES {
            if let Some(old) = self.order.pop_front() {
                self.by_id.remove(&old);
            }
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let editor =
        std::env::var("ZEYN_PLAYGROUND_EDITOR").unwrap_or_else(|_| "127.0.0.1:3120".into());
    let preview =
        std::env::var("ZEYN_PLAYGROUND_PREVIEW").unwrap_or_else(|_| "127.0.0.1:3121".into());
    let play = Arc::new(Play {
        image: std::env::var("ZEYN_PLAYGROUND_IMAGE")
            .unwrap_or_else(|_| "zeyn-playground-sandbox".into()),
        editor_origin: format!("http://{editor}"),
        preview_origin: format!("http://{preview}"),
        pages: Mutex::new(Pages::default()),
        slots: Semaphore::new(SLOTS),
        waiting: AtomicUsize::new(0),
        runs: Mutex::new(HashMap::new()),
    });
    let editor_app = Router::new()
        .route("/", get(editor_page))
        .route("/play.js", get(script))
        .route("/play.css", get(style))
        .route("/run", post(run))
        .layer(DefaultBodyLimit::max(check::MAX_BODY + 1024))
        .with_state(play.clone());
    let runtime = zeyn::App::new("Playground")
        .router()
        .map_err(|e| std::io::Error::other(e.join("; ")))?;
    let preview_app = runtime.route(
        "/p/{id}",
        get({
            let play = play.clone();
            move |id| preview_page(play, id)
        }),
    );
    let editor_listener = tokio::net::TcpListener::bind(&editor).await?;
    let preview_listener = tokio::net::TcpListener::bind(&preview).await?;
    println!("playground editor on http://{editor}, previews on http://{preview}");
    let editor_server = axum::serve(
        editor_listener,
        editor_app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    let preview_server = axum::serve(preview_listener, preview_app);
    tokio::try_join!(async { editor_server.await }, async {
        preview_server.await
    })?;
    Ok(())
}

/// The editor page. Its frame may load only the preview origin.
async fn editor_page(State(play): State<Arc<Play>>) -> Response {
    let csp = format!(
        "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-src {}; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        play.preview_origin
    );
    asset(
        include_str!("../assets/index.html"),
        "text/html; charset=utf-8",
        &csp,
    )
}

async fn script() -> Response {
    asset(
        include_str!("../assets/play.js"),
        "text/javascript; charset=utf-8",
        "default-src 'none'",
    )
}

async fn style() -> Response {
    asset(
        include_str!("../assets/play.css"),
        "text/css; charset=utf-8",
        "default-src 'none'",
    )
}

/// A static file with its type and policy.
fn asset(body: &'static str, kind: &'static str, csp: &str) -> Response {
    let mut response = body.into_response();
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    if let Ok(v) = HeaderValue::from_str(csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// The body of a run request.
#[derive(Deserialize)]
struct RunRequest {
    body: String,
}

/// Checks, rate limits, queues and runs one screen body. Answers 200 with the preview address,
/// or 200 with `error` (and the compiler `log`) when the code is at fault; 429, 503 and 500 are
/// for the playground itself.
async fn run(
    State(play): State<Arc<Play>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    axum::Json(request): axum::Json<RunRequest>,
) -> Response {
    if let Err(problem) = check::body(&request.body) {
        return answer(StatusCode::OK, json!({ "error": problem }));
    }
    let source = runner::source(&request.body);
    let id = hash(&source);
    let preview = format!("{}/p/{id}", play.preview_origin);
    if play
        .pages
        .lock()
        .expect("pages lock")
        .get(&id)
        .is_some()
    {
        return answer(
            StatusCode::OK,
            json!({ "preview": preview, "cached": true }),
        );
    }
    if !allow(&play, peer.ip()) {
        return answer(
            StatusCode::TOO_MANY_REQUESTS,
            json!({ "error": "too many runs from this address; wait a minute" }),
        );
    }
    if play
        .waiting
        .fetch_add(1, Ordering::SeqCst)
        >= QUEUE
    {
        play.waiting
            .fetch_sub(1, Ordering::SeqCst);
        return answer(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "the playground is busy; try again soon" }),
        );
    }
    let permit = play.slots.acquire().await;
    play.waiting
        .fetch_sub(1, Ordering::SeqCst);
    let Ok(_permit) = permit else {
        return answer(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({ "error": "closed" }),
        );
    };
    let started = Instant::now();
    let name = format!("zeyn-play-{}", &id[..16]);
    let result = runner::run(&play.image, &name, &source).await;
    let total_ms = started.elapsed().as_millis();
    match result {
        Ok(outcome) => {
            play.pages
                .lock()
                .expect("pages lock")
                .put(id, Arc::from(outcome.page));
            println!("run {name}: {} total_ms={total_ms}", outcome.stats);
            answer(
                StatusCode::OK,
                json!({ "preview": preview, "cached": false, "stats": outcome.stats, "total_ms": total_ms }),
            )
        }
        Err(runner::Failure::Compile(log)) => answer(
            StatusCode::OK,
            json!({ "error": "the code did not compile", "log": log }),
        ),
        Err(runner::Failure::Limit) => answer(
            StatusCode::OK,
            json!({ "error": "the run went over its time or memory limit" }),
        ),
        Err(runner::Failure::Sandbox(why)) => {
            eprintln!("sandbox failure {name}: {why}");
            answer(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "the sandbox failed" }),
            )
        }
    }
}

/// A JSON answer.
fn answer(status: StatusCode, body: serde_json::Value) -> Response {
    (status, axum::Json(body)).into_response()
}

/// The hex SHA-256 of the full source, the id of its page.
fn hash(source: &str) -> String {
    Sha256::digest(source.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Counts a run of `ip` in its window; false when the window is used up.
fn allow(play: &Play, ip: IpAddr) -> bool {
    let now = Instant::now();
    let mut runs = play.runs.lock().expect("runs lock");
    if runs.len() >= MAX_ADDRESSES && !runs.contains_key(&ip) {
        runs.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        if runs.len() >= MAX_ADDRESSES {
            return false;
        }
    }
    let entry = runs.entry(ip).or_insert((now, 0));
    if now.duration_since(entry.0) >= WINDOW {
        *entry = (now, 0);
    }
    entry.1 += 1;
    entry.1 <= RUNS_PER_WINDOW
}

/// A rendered page, on the preview origin only, framed only by the editor.
async fn preview_page(play: Arc<Play>, Path(id): Path<String>) -> Response {
    let page = play
        .pages
        .lock()
        .expect("pages lock")
        .get(&id);
    let Some(page) = page else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    let csp = format!(
        "default-src 'none'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors {}",
        play.editor_origin
    );
    if let Ok(v) = HeaderValue::from_str(&csp) {
        headers.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    (headers, page.to_string()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play() -> Play {
        Play {
            image: String::new(),
            editor_origin: String::new(),
            preview_origin: String::new(),
            pages: Mutex::new(Pages::default()),
            slots: Semaphore::new(SLOTS),
            waiting: AtomicUsize::new(0),
            runs: Mutex::new(HashMap::new()),
        }
    }

    #[test]
    fn an_address_gets_a_fixed_number_of_runs_per_window() {
        let play = play();
        let ip: IpAddr = [10, 0, 0, 1].into();
        for _ in 0..RUNS_PER_WINDOW {
            assert!(allow(&play, ip));
        }
        assert!(!allow(&play, ip));
        assert!(allow(&play, [10, 0, 0, 2].into()));
    }

    #[test]
    fn the_page_cache_keeps_the_newest_pages() {
        let mut pages = Pages::default();
        for i in 0..CACHE_PAGES + 5 {
            pages.put(i.to_string(), Arc::from("x"));
        }
        assert_eq!(pages.by_id.len(), CACHE_PAGES);
        assert!(pages.get("0").is_none());
        assert!(
            pages
                .get(&(CACHE_PAGES + 4).to_string())
                .is_some()
        );
    }
}
