//! Runtime assets and page head additions for the enterprise features.

use super::{App, AppState, asset};
use crate::ctx::BoxFuture;
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{attr, escape};
use crate::security::IpRange;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use serde_json::json;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const JS: &str = include_str!("../../assets/enterprise.js");
const CSS: &str = include_str!("../../assets/enterprise.css");
const DEV_JS: &str = include_str!("../../assets/dev.js");

/// Development mode, set by `zeyn dev` with `ZEYN_DEV=1`: the page follows rebuilds without a
/// page load. Read once; never set it in production.
pub(super) fn dev_mode() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("ZEYN_DEV").is_ok_and(|v| v == "1"))
}

/// `GET /_zeyn/dev/events` in development: one `build` event with this binary's build id, then a
/// comment every 15 seconds to keep the connection open. When the server restarts the browser
/// reconnects (after 300 ms) and compares the build id.
async fn dev_events() -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let first = format!("retry: 300\nevent: build\ndata: {}\n\n", build_id());
        if tx.send(Ok(first.into())).await.is_err() {
            return;
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            if tx
                .send(Ok(": ping\n\n".into()))
                .await
                .is_err()
            {
                return;
            }
        }
    });
    let mut response = axum::body::Body::from_stream(super::Chunks(rx)).into_response();
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Static assets that skip IP rules and rate limits like the other runtime files.
pub(super) fn is_asset(path: &str) -> bool {
    path == "/_zeyn/enterprise.js" || path == "/_zeyn/enterprise.css" || path == "/_zeyn/dev.js"
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
    jobs: Option<crate::jobs::Jobs>,
    jobs_permission: Option<&'static str>,
    /// The last readiness result and when it was taken. Requests within [`READY_CACHE`] share it,
    /// and only one request at a time runs the checks.
    ready: tokio::sync::Mutex<Option<(std::time::Instant, Vec<&'static str>)>>,
    allow_dev_auth: bool,
}

/// The startup problem of a development authenticator in a release build, unless allowed.
pub(super) fn dev_auth_problem(release: bool, development: bool, allowed: bool) -> Option<String> {
    (release && development && !allowed).then(|| {
        "DevAuth signs every request in with full access, so it does not run in release builds
  = help: use a real authenticator in release builds (OidcAuth, JwtAuth, ForwardAuth or App::accounts), for example only under `#[cfg(debug_assertions)]`
  = help: a demo that must run in a release build can call `App::allow_dev_auth()`"
            .into()
    })
}

impl Extras {
    /// Whether an address may reach the health endpoints despite `allow_only_ips`.
    pub(super) fn probe_ip(&self, ip: std::net::IpAddr) -> bool {
        self.probe_ips
            .iter()
            .any(|r| r.contains(ip))
    }
}

/// Longest time one readiness check may take before it counts as failed.
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a readiness result is reused, so a flood of probes cannot load the checked services.
pub const READY_CACHE: Duration = Duration::from_secs(1);

/// `/_zeyn/health` and `/_zeyn/ready`; IP rules apply (see `App::probe_ips`).
pub(super) fn is_probe(path: &str) -> bool {
    path == "/_zeyn/health" || path == "/_zeyn/ready"
}

/// Whether a probe skips the rate limits: health always (it does no work), readiness only from
/// the probe addresses, because it runs the checks.
pub(super) fn probe_unlimited(extras: &Extras, path: &str, ip: Option<std::net::IpAddr>) -> bool {
    path == "/_zeyn/health" || (path == "/_zeyn/ready" && ip.is_some_and(|ip| extras.probe_ip(ip)))
}

impl App {
    /// Lets `DevAuth` (or another development authenticator) run in a release build. Only for
    /// demos that are never reachable by others: every request gets full access.
    pub fn allow_dev_auth(mut self) -> Self {
        self.extras.allow_dev_auth = true;
        self
    }

    /// The problem `check` reports for a development authenticator in this build.
    pub(super) fn dev_auth_check(&self) -> Option<String> {
        dev_auth_problem(
            !cfg!(debug_assertions),
            self.auth
                .as_ref()
                .is_some_and(|a| a.is_development()),
            self.extras.allow_dev_auth,
        )
    }

    /// Adds a readiness check, such as a database ping. `GET /_zeyn/ready` answers `200` when every
    /// check returns `true` within five seconds, otherwise `503` with the names of the failed
    /// checks. The result is reused for [`READY_CACHE`], and the endpoint counts toward the rate
    /// limits except from [`App::probe_ips`]. Check names are shown to anyone who can reach the endpoint; keep them plain
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
        self.extras
            .readiness
            .push((name, Arc::new(move || Box::pin(check()))));
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

    /// Adds a background job that runs in this process. See [`crate::Schedule`] for intervals,
    /// triggers and retries. Actions trigger jobs through the injectable [`crate::Jobs`] service.
    /// Jobs start with the router and stop with [`App::serve_until`].
    pub fn job<F, Fut>(
        mut self,
        name: &'static str,
        schedule: crate::jobs::Schedule,
        run: F,
    ) -> Self
    where
        F: Fn(Ctx) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        let jobs = self
            .extras
            .jobs
            .get_or_insert_with(crate::jobs::Jobs::default)
            .clone();
        jobs.add(name, schedule, run);
        self.services.insert(jobs);
        self
    }

    /// Adds a job whose queued runs carry a payload, such as a record id. Actions queue runs with
    /// [`crate::Jobs::enqueue`]; the schedule gives the retries (and an interval, if any).
    pub fn queued_job<F, Fut>(
        mut self,
        name: &'static str,
        schedule: crate::jobs::Schedule,
        run: F,
    ) -> Self
    where
        F: Fn(Ctx, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        let jobs = self
            .extras
            .jobs
            .get_or_insert_with(crate::jobs::Jobs::default)
            .clone();
        jobs.add_queued(name, schedule, run);
        self.services.insert(jobs);
        self
    }

    /// Keeps queued runs and the next run of every scheduled job in `store`, such as
    /// `SqlJobStore`, so they survive restarts and run once across servers. In memory unless set.
    /// Call [`App::job_concurrency`] first when you set both.
    pub fn job_store(mut self, store: impl crate::jobs::JobStore) -> Self {
        let jobs = self
            .extras
            .jobs
            .get_or_insert_with(crate::jobs::Jobs::default)
            .clone();
        if let Err(e) = jobs.set_store(store) {
            self.setup_problems
                .push(format!("job_store: {e:?}"));
        }
        self.services.insert(jobs);
        self
    }

    /// Most jobs that run at the same time; four unless set. Call it before adding jobs.
    pub fn job_concurrency(mut self, limit: usize) -> Self {
        match &self.extras.jobs {
            Some(jobs) if !jobs.is_empty() => {
                self.setup_problems
                    .push("job_concurrency must be set before the first job is added".into());
            }
            _ => {
                let jobs = crate::jobs::Jobs::with_limit(limit);
                self.services.insert(jobs.clone());
                self.extras.jobs = Some(jobs);
            }
        }
        self
    }

    /// Publishes job status as JSON at `GET /_zeyn/jobs` to users with this permission. Off
    /// unless set.
    pub fn jobs_permission(mut self, permission: &'static str) -> Self {
        self.extras.jobs_permission = Some(permission);
        self
    }

    /// Serves the app until `shutdown` completes, then stops accepting requests, lets running
    /// requests finish and gives running jobs up to 30 seconds. Pass a signal such as Ctrl+C.
    ///
    /// ```ignore
    /// app.serve_until("0.0.0.0:3000", async { let _ = tokio::signal::ctrl_c().await; }).await
    /// ```
    pub async fn serve_until(
        self,
        addr: &str,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let jobs = self.extras.jobs.clone();
        let flush = self.start_telemetry();
        let router = self.router().map_err(|problems| {
            for p in &problems {
                tracing::error!("{p}");
            }
            std::io::Error::other("the app has problems; see the errors above")
        })?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("Zeyn is listening on http://{}", listener.local_addr()?);
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await?;
        if let Some(jobs) = jobs
            && !jobs
                .shutdown(Duration::from_secs(30))
                .await
        {
            tracing::warn!("some jobs were still running at shutdown");
        }
        if let Some(f) = flush {
            f.shutdown().await;
        }
        Ok(())
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
    if let Some(d) = st
        .extras
        .latency
        .filter(|_| !is_probe(path))
    {
        tokio::time::sleep(d).await;
    }
}

#[cfg(not(debug_assertions))]
pub(super) async fn delay(_st: &AppState, _path: &str) {}

/// Starts the app's jobs when a Tokio runtime is running. Each run gets a context with the app's
/// services, configuration and default language, as user `job:<name>` with no permissions.
pub(super) fn start_jobs(st: &Arc<AppState>) {
    let Some(jobs) = &st.extras.jobs else { return };
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    let (services, i18n, config) = (st.services.clone(), st.i18n.clone(), st.config.clone());
    jobs.start(move |name| {
        let locale = i18n.default_locale().to_string();
        Ctx::new(
            crate::ctx::Identity::user(format!("job:{name}")),
            locale,
            services.clone(),
            i18n.clone(),
        )
        .with_config(config.clone())
    });
}

/// Job status for users with the jobs permission.
async fn jobs_status(State(st): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> Response {
    let ctx = st.ctx(&headers);
    let (Some(jobs), Some(permission)) = (&st.extras.jobs, st.extras.jobs_permission) else {
        return st.error_response(&ctx, super::Failure::App(crate::Error::NotFound));
    };
    if !ctx
        .identity()
        .can(&crate::Access::Permission(permission))
    {
        return st.error_response(&ctx, super::Failure::App(crate::Error::Forbidden));
    }
    no_store(axum::Json(json!({ "jobs": jobs.status() })).into_response())
}

fn no_store(mut r: Response) -> Response {
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// Liveness: the process runs and answers.
async fn health() -> Response {
    no_store(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok").into_response())
}

/// Readiness: every registered check passed. Failed checks are named; their errors are not shown.
/// The result is reused for [`READY_CACHE`].
async fn ready(State(st): State<Arc<AppState>>) -> Response {
    let mut last = st.extras.ready.lock().await;
    let failed = match last.as_ref() {
        Some((at, failed)) if at.elapsed() < READY_CACHE => failed.clone(),
        _ => {
            let tasks: Vec<_> = st
                .extras
                .readiness
                .iter()
                .map(|(name, check)| {
                    let fut = check();
                    let task = tokio::spawn(async move {
                        tokio::time::timeout(CHECK_TIMEOUT, fut)
                            .await
                            .unwrap_or(false)
                    });
                    (*name, task)
                })
                .collect();
            let mut failed = Vec::new();
            for (name, task) in tasks {
                if !task.await.unwrap_or(false) {
                    failed.push(name);
                }
            }
            *last = Some((std::time::Instant::now(), failed.clone()));
            failed
        }
    };
    drop(last);
    let r = if failed.is_empty() {
        (StatusCode::OK, axum::Json(json!({ "status": "ready" }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({ "status": "not_ready", "failed": failed })),
        )
            .into_response()
    };
    no_store(r)
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    let router = router
        .route("/_zeyn/health", get(health))
        .route("/_zeyn/ready", get(ready))
        .route("/_zeyn/jobs", get(jobs_status))
        .route(
            "/_zeyn/enterprise.js",
            get(|| async { asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/enterprise.css",
            get(|| async { asset(CSS, "text/css; charset=utf-8") }),
        );
    if dev_mode() {
        router
            .route(
                "/_zeyn/dev.js",
                get(|| async { asset(DEV_JS, "text/javascript; charset=utf-8") }),
            )
            .route("/_zeyn/dev/events", get(dev_events))
    } else {
        router
    }
}

/// Asks a running app at `addr` whether it is alive (`GET /_zeyn/health`), for container health
/// checks in images without a shell or curl: `my-app health` exits non zero when it is not.
/// A wildcard address (`0.0.0.0`, `[::]`) is checked on the loopback address. Std only, no
/// runtime, one short connection with a two second limit.
///
/// ```no_run
/// zeyn::probe("0.0.0.0:3000").expect("the app answers");
/// ```
pub fn probe(addr: &str) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let mut target: SocketAddr = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("no address"))?;
    if target.ip().is_unspecified() {
        target.set_ip(if target.is_ipv4() {
            [127, 0, 0, 1].into()
        } else {
            std::net::Ipv6Addr::LOCALHOST.into()
        });
    }
    let limit = Duration::from_secs(2);
    let mut stream = TcpStream::connect_timeout(&target, limit)?;
    stream.set_read_timeout(Some(limit))?;
    stream.write_all(b"GET /_zeyn/health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut head = [0u8; 16];
    let n = stream.read(&mut head)?;
    match head
        .get(..n)
        .and_then(|h| std::str::from_utf8(h).ok())
    {
        Some(line) if line.starts_with("HTTP/1.") && line.get(9..12) == Some("200") => Ok(()),
        _ => Err(std::io::Error::other("health check failed")),
    }
}

/// Header that carries the build id on every response.
pub(super) const BUILD_HEADER: &str = "x-zeyn-build";

/// FNV-1a, 64 bit. Only used to tell builds apart, not for security.
/// `?v=` and the [`build_id`], appended to the URL of every framework script and stylesheet a
/// page links, so the browser keeps them for a year without asking again (see `static_files`).
/// A new build changes the URLs.
pub(super) fn asset_version() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| format!("?v={}", build_id()))
}

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
        let from_env = std::env::var("ZEYN_BUILD_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| {
                !v.is_empty()
                    && v.len() <= 64
                    && v.chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
            });
        from_env.unwrap_or_else(|| {
            let seed = 0xcbf2_9ce4_8422_2325;
            let hash = match std::env::current_exe().and_then(std::fs::read) {
                Ok(bytes) => fnv(&bytes, seed),
                Err(_) => [
                    super::JS,
                    super::GRID_JS,
                    super::CSS,
                    JS,
                    CSS,
                    env!("CARGO_PKG_VERSION"),
                ]
                .iter()
                .fold(seed, |h, s| fnv(s.as_bytes(), h)),
            };
            format!("{hash:016x}")
        })
    })
}

/// Adds the build id header to a response, so the runtime notices a new deploy.
pub(super) async fn stamp(mut response: Response) -> Response {
    if let Ok(v) = HeaderValue::from_str(build_id()) {
        response
            .headers_mut()
            .insert(BUILD_HEADER, v);
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
    "session.mfa_title",
    "session.mfa_body",
    "session.mfa_open",
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
    let texts: serde_json::Map<String, serde_json::Value> = TEXTS
        .iter()
        .map(|k| {
            (
                k.to_string(),
                ctx.t(&Text::key(format!("zeyn.{k}")))
                    .into(),
            )
        })
        .collect();
    format!(
        r#"{session}<meta name="zeyn-build" content="{build}"><meta name="zeyn-texts" content="{texts}"><link rel="stylesheet" href="{base}/_zeyn/enterprise.css{v}"><script type="module" src="{base}/_zeyn/enterprise.js{v}"></script>{dev}"#,
        dev = if dev_mode() {
            format!(
                r#"<meta name="zeyn-dev" content="1"><script type="module" src="{}/_zeyn/dev.js{}"></script>"#,
                escape(&st.prefix),
                asset_version()
            )
        } else {
            String::new()
        },
        texts = attr(&texts),
        build = escape(build_id()),
        session = super::session::head(st, ctx),
        base = escape(&st.prefix),
        v = asset_version(),
    ) + &accounts_head(st)
}

/// The passkey script of the accounts system, when it is on.
fn accounts_head(st: &AppState) -> String {
    #[cfg(feature = "accounts")]
    if st.sign_in.accounts.is_some() {
        return format!(
            r#"<script type="module" src="{}/_zeyn/account/accounts.js{}"></script>"#,
            escape(&st.prefix),
            asset_version()
        );
    }
    let _ = st;
    String::new()
}

#[cfg(test)]
mod tests {
    use super::dev_auth_problem;

    #[test]
    fn development_sign_in_is_refused_in_release_builds_unless_allowed() {
        assert!(dev_auth_problem(true, true, false).is_some_and(|p| p.contains("allow_dev_auth")));
        assert!(dev_auth_problem(true, true, true).is_none());
        assert!(
            dev_auth_problem(false, true, false).is_none(),
            "debug builds keep it"
        );
        assert!(dev_auth_problem(true, false, false).is_none());
    }
}
