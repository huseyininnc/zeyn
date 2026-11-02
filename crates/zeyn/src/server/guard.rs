//! Middleware that runs before and around every handler: IP rules, rate limits and the write
//! limit, the request span, and the security headers on every response.

use super::*;

/// Content security policy of an app without `App::csp`: no inline scripts, no framing,
/// everything from this origin. [`crate::csp::Csp::header`] builds the same text.
pub(super) const DEFAULT_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// Framework endpoints whose `POST` changes data or runs an action. They count against the
/// write limit (`App::write_limit`) besides the general one. A new endpoint that writes, runs an
/// action or costs money goes here. Published API actions and the assistant are matched by prefix,
/// and file uploads (`POST /_zeyn/files/{store}`) by `files::is_upload`.
const WRITE_PATHS: [&str; 14] = [
    "/_zeyn/form",
    "/_zeyn/reveal",
    "/_zeyn/data",
    "/_zeyn/import",
    "/_zeyn/mcp",
    "/_zeyn/menu",
    "/_zeyn/grid/edit",
    "/_zeyn/grid/run",
    "/_zeyn/grid/bulk",
    "/_zeyn/grid/view",
    "/_zeyn/command",
    "/_zeyn/dock",
    "/_zeyn/theme",
    "/_zeyn/media/position",
];

/// Framework endpoints whose `GET` costs as much as a write: exports, printed documents and the
/// sitemap (with its parts under `/sitemaps/`). They count against the write limit too.
const COSTLY_GETS: [&str; 3] = ["/_zeyn/grid/export", "/_zeyn/pdf", "/sitemap.xml"];

/// Whether a path is a static asset of the runtime, served without IP rules, rate limits or sign
/// in so that error and sign in pages still load their scripts and styles.
fn is_public_asset(path: &str) -> bool {
    matches!(
        path,
        "/_zeyn/zeyn.js" | "/_zeyn/zeyn.css" | "/_zeyn/nav.js" | "/_zeyn/theme.css" | "/favicon.ico" | "/llms.txt"
    ) || ops::is_asset(path)
        || ui::ASSETS.contains(&path)
        || media::ASSETS.contains(&path)
        || pro::ASSETS.contains(&path)
        || files::ASSETS.contains(&path)
        || charts::ASSETS.contains(&path)
        || site::ASSETS.contains(&path)
        || is_assistant_asset(path)
        || is_island_asset(path)
}

/// Whether a path is one of the assistant's assets, served without sign in like the runtime.
fn is_assistant_asset(path: &str) -> bool {
    #[cfg(feature = "assistant")]
    return assistant::ASSETS.contains(&path);
    #[cfg(not(feature = "assistant"))]
    {
        let _ = path;
        false
    }
}

/// Whether a path is the island loader, a built island file or the live script, served without
/// sign in.
fn is_island_asset(path: &str) -> bool {
    #[cfg(feature = "realtime")]
    if realtime::is_asset(path) {
        return true;
    }
    #[cfg(feature = "islands")]
    return islands::is_asset(path);
    #[cfg(not(feature = "islands"))]
    {
        let _ = path;
        false
    }
}

/// Every `X-Forwarded-For` line of a request as one chain, in order. Proxies may add their own
/// line instead of extending the last one, so reading only the first would trust the client's.
pub(super) fn forwarded_for(headers: &HeaderMap) -> Option<std::borrow::Cow<'_, str>> {
    let mut lines = headers.get_all("x-forwarded-for").iter().filter_map(|v| v.to_str().ok());
    let first = lines.next()?;
    match lines.next() {
        None => Some(std::borrow::Cow::Borrowed(first)),
        Some(second) => {
            let mut chain = format!("{first},{second}");
            for line in lines {
                chain.push(',');
                chain.push_str(line);
            }
            Some(std::borrow::Cow::Owned(chain))
        }
    }
}

/// IP rules and rate limits, before any handler runs.
pub(super) async fn guard_request(
    State(st): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if is_public_asset(req.uri().path()) {
        return next.run(req).await;
    }
    let (parts, body) = req.into_parts();
    let path = parts.uri.path();
    let headers = &parts.headers;
    ops::delay(&st, path).await;
    if let Some(ready) = st.auth.as_ref().and_then(|a| a.prepare(headers)) {
        ready.await;
    }
    let peer = parts.extensions.get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip());
    let forwarded = forwarded_for(headers);
    let ip = client_ip(peer, forwarded.as_deref(), &st.trusted);
    let probe = ops::is_probe(path);
    let unlimited = ops::probe_unlimited(&st.extras, path, ip);
    let blocked = match ip {
        Some(ip) => {
            st.deny.iter().any(|r| r.contains(ip))
                || (!st.allow.is_empty()
                    && !st.allow.iter().any(|r| r.contains(ip))
                    && !(probe && st.extras.probe_ip(ip)))
        }
        None => !st.allow.is_empty(),
    };
    if blocked {
        let ctx = st.ctx(headers);
        return problem(&ctx, Failure::App(Error::Forbidden));
    }
    let is_write = (parts.method == axum::http::Method::GET
        && (COSTLY_GETS.contains(&path) || path.starts_with("/sitemaps/")))
        || parts.method == axum::http::Method::POST
            && (WRITE_PATHS.contains(&path)
                || files::is_upload(path)
                || path.starts_with("/_zeyn/assistant/")
                || path.starts_with("/_zeyn/account/")
                || path.strip_prefix(st.api_prefix.as_str()).is_some_and(|rest| rest.starts_with('/')));
    let limits = if unlimited {
        [(None, "all"), (None, "write")]
    } else {
        [(st.rate, "all"), (if is_write { st.write_rate } else { None }, "write")]
    };
    for (limit, scope) in limits {
        let Some(limit) = limit else { continue };
        let ip_key = ip.map(|i| i.to_string()).unwrap_or_else(|| "unknown".into());
        let key = match limit.key {
            RateKey::Ip => format!("{scope}:ip:{ip_key}"),
            RateKey::User => match st.auth.as_ref().and_then(|a| a.authenticate(headers)).and_then(|i| i.user) {
                Some(user) => format!("{scope}:user:{user}"),
                None => format!("{scope}:ip:{ip_key}"),
            },
        };
        if let Err(wait) = st.rate_store.take(&key, &limit).await {
            let ctx = st.ctx(headers);
            let mut response = problem(&ctx, Failure::App(Error::Denied(Text::key("zeyn.error.rate_limited"))));
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Some(body) = response.headers_mut().get_mut(header::CONTENT_TYPE) {
                *body = HeaderValue::from_static("application/problem+json");
            }
            let secs = wait.as_secs_f64().ceil().max(1.0) as u64;
            if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, v);
            }
            return response;
        }
    }
    next.run(axum::extract::Request::from_parts(parts, body)).await
}

/// One `request` span per request: method, path without the query string, status and duration,
/// plus the call's kind, screen, action, user, tenant and outcome once the handler knows them.
pub(super) async fn trace_request(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    use tracing::Instrument;
    use tracing::field::Empty;
    let span = tracing::info_span!(
        "request",
        method = %req.method(),
        path = req.uri().path(),
        status = Empty,
        ms = Empty,
        kind = Empty,
        screen = Empty,
        action = Empty,
        user = Empty,
        tenant = Empty,
        ok = Empty,
        error = Empty,
    );
    if span.is_disabled() {
        return next.run(req).await;
    }
    let started = Instant::now();
    let response = next.run(req).instrument(span.clone()).await;
    span.record("status", response.status().as_u16());
    span.record("ms", started.elapsed().as_millis() as u64);
    response
}

/// Security headers on every response. The policy is the one built at startup from `App::csp`.
pub(super) async fn security_headers(State(st): State<Arc<AppState>>, mut response: Response) -> Response {
    let h = response.headers_mut();
    h.entry(header::CONTENT_SECURITY_POLICY).or_insert_with(|| st.csp_header.clone());
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("same-origin"));
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}
