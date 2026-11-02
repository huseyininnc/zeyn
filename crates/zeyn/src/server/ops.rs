//! Runtime assets and page head additions for the enterprise features.

use super::{App, AppState, asset};
use crate::ctx::BoxFuture;
use crate::security::IpRange;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde_json::json;
use std::future::Future;
use std::time::Duration;
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{attr, escape};
use axum::Router;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::get;
use std::sync::{Arc, OnceLock};

const JS: &str = include_str!("../../assets/enterprise.js");
const CSS: &str = include_str!("../../assets/enterprise.css");

/// Static assets that skip IP rules and rate limits like the other runtime files.
pub(super) fn is_asset(path: &str) -> bool {
    path == "/_zeyn/enterprise.js" || path == "/_zeyn/enterprise.css"
}

type Check = Arc<dyn Fn() -> BoxFuture<bool> + Send + Sync>;

/// App settings of the enterprise features.
#[derive(Default)]
pub(super) struct Extras {
    readiness: Vec<(&'static str, Check)>,
    probe_ips: Vec<IpRange>,
    #[cfg(debug_assertions)]
    pub(super) latency: Option<Duration>,
    pub(super) session: super::session::Settings,
}

impl Extras {
    /// Whether an address may reach the health endpoints despite `allow_only_ips`.
    pub(super) fn probe_ip(&self, ip: std::net::IpAddr) -> bool {
        self.probe_ips.iter().any(|r| r.contains(ip))
    }
}

/// Longest time one readiness check may take before it counts as failed.
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// `/_zeyn/health` and `/_zeyn/ready`: never rate limited; IP rules apply (see `App::probe_ips`).
pub(super) fn is_probe(path: &str) -> bool {
    path == "/_zeyn/health" || path == "/_zeyn/ready"
}

impl App {
    /// Adds a readiness check, such as a database ping. `GET /_zeyn/ready` answers `200` when every
    /// check returns `true` within five seconds, otherwise `503` with the names of the failed
    /// checks. Check names are shown to anyone who can reach the endpoint; keep them plain
    /// (`database`, `license`) and never put secrets in them.
    ///
    /// ```ignore
    /// App::new("Billing").readiness("database", move || { let pool = pool.clone(); async move { pool.ping().await.is_ok() } })
    /// ```
    pub fn readiness<F, Fut>(mut self, name: &'static str, check: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        self.extras.readiness.push((name, Arc::new(move || Box::pin(check()))));
        self
    }

    /// Delays every call by `delay` so developers feel a slow network: screens, grids, forms, API
    /// and runtime calls, not static assets or health probes. Only in debug builds; in release
    /// builds this does nothing and the delay code is not compiled in.
    pub fn dev_latency(self, delay: Duration) -> Self {
        #[cfg(debug_assertions)]
        {
            let mut app = self;
            app.extras.latency = Some(delay);
            app
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = delay;
            self
        }
    }

    /// Addresses of probes (a load balancer, an orchestrator) that may reach `/_zeyn/health` and
    /// `/_zeyn/ready` even when `allow_only_ips` would refuse them. Denied addresses stay denied.
    pub fn probe_ips<I, S>(mut self, ranges: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.extras.probe_ips = self.ranges("probe_ips", ranges);
        self
    }
}

/// Waits for the simulated latency, if set. Compiled out of release builds.
#[cfg(debug_assertions)]
pub(super) async fn delay(st: &AppState, path: &str) {
    if let Some(d) = st.extras.latency.filter(|_| !is_probe(path)) {
        tokio::time::sleep(d).await;
    }
}

#[cfg(not(debug_assertions))]
pub(super) async fn delay(_st: &AppState, _path: &str) {}

fn no_store(mut r: Response) -> Response {
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// Liveness: the process runs and answers.
async fn health() -> Response {
    no_store(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok").into_response())
}

/// Readiness: every registered check passed. Failed checks are named; their errors are not shown.
async fn ready(State(st): State<Arc<AppState>>) -> Response {
    let tasks: Vec<_> = st
        .extras
        .readiness
        .iter()
        .map(|(name, check)| {
            let fut = check();
            (*name, tokio::spawn(async move { tokio::time::timeout(CHECK_TIMEOUT, fut).await.unwrap_or(false) }))
        })
        .collect();
    let mut failed = Vec::new();
    for (name, task) in tasks {
        if !task.await.unwrap_or(false) {
            failed.push(name);
        }
    }
    let r = if failed.is_empty() {
        (StatusCode::OK, axum::Json(json!({ "status": "ready" }))).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, axum::Json(json!({ "status": "not_ready", "failed": failed }))).into_response()
    };
    no_store(r)
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/health", get(health))
        .route("/_zeyn/ready", get(ready))
        .route("/_zeyn/enterprise.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/enterprise.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
}

/// Header that carries the build id on every response.
pub(super) const BUILD_HEADER: &str = "x-zeyn-build";

/// FNV-1a, 64 bit. Only used to tell builds apart, not for security.
fn fnv(bytes: &[u8], mut hash: u64) -> u64 {
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Id of the running build: `ZEYN_BUILD_ID` when set (letters, digits, `.`, `_`, `-`, at most 64),
/// otherwise a hash of the executable, or of the runtime assets when it cannot be read.
pub fn build_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let from_env = std::env::var("ZEYN_BUILD_ID").ok().map(|v| v.trim().to_string()).filter(|v| {
            !v.is_empty() && v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
        });
        from_env.unwrap_or_else(|| {
            let seed = 0xcbf2_9ce4_8422_2325;
            let hash = match std::env::current_exe().and_then(std::fs::read) {
                Ok(bytes) => fnv(&bytes, seed),
                Err(_) => [super::JS, super::CSS, JS, CSS, env!("CARGO_PKG_VERSION")].iter().fold(seed, |h, s| fnv(s.as_bytes(), h)),
            };
            format!("{hash:016x}")
        })
    })
}

/// Adds the build id header to a response, so the runtime notices a new deploy.
pub(super) async fn stamp(mut response: Response) -> Response {
    if let Ok(v) = HeaderValue::from_str(build_id()) {
        response.headers_mut().insert(BUILD_HEADER, v);
    }
    response
}

/// Texts the runtime shows, in the user's language.
const TEXTS: &[&str] = &[
    "deploy.new_version",
    "deploy.reload",
    "deploy.later",
    "draft.restored",
    "draft.discard",
    "import.too_large",
    "import.reading",
    "session.idle_title",
    "session.idle_body",
    "session.stay",
    "session.signed_out_title",
    "session.signed_out_body",
    "session.sign_in",
    "session.reauth_title",
    "session.reauth_body",
    "session.reauth_open",
    "session.reauth_continue",
    "privacy.revealed",
    "privacy.reveal_failed",
    "privacy.reason_title",
    "privacy.reason_explain",
    "privacy.reason_label",
    "privacy.reason_continue",
    "privacy.reason_cancel",
    "privacy.reason_invalid",
];

/// Extra `<head>` markup: the enterprise runtime and its texts.
pub(super) fn head(st: &AppState, ctx: &Ctx) -> String {
    let texts: serde_json::Map<String, serde_json::Value> =
        TEXTS.iter().map(|k| (k.to_string(), ctx.t(&Text::key(format!("zeyn.{k}"))).into())).collect();
    format!(
        r#"{session}<meta name="zeyn-build" content="{build}"><meta name="zeyn-texts" content="{texts}"><link rel="stylesheet" href="{base}/_zeyn/enterprise.css"><script type="module" src="{base}/_zeyn/enterprise.js"></script>"#,
        texts = attr(&texts),
        build = escape(build_id()),
        session = super::session::head(st, ctx),
        base = escape(&st.prefix),
    )
}
