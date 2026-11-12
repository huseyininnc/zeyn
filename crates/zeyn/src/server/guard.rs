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
const WRITE_PATHS: [&str; 18] = [
    "/_zeyn/form",
    "/_zeyn/reveal",
    "/_zeyn/data",
    "/_zeyn/node",
    "/_zeyn/nodes",
    "/_zeyn/stream",
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
    "/_zeyn/density",
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
        "/_zeyn/zeyn.js"
            | "/_zeyn/grid.js"
            | "/_zeyn/zeyn.css"
            | "/_zeyn/icons.css"
            | "/_zeyn/nav.js"
            | "/_zeyn/theme.css"
            | "/favicon.ico"
            | "/llms.txt"
    ) || ops::is_asset(path)
        || ui::ASSETS.contains(&path)
        || media::ASSETS.contains(&path)
        || pro::ASSETS.contains(&path)
        || files::ASSETS.contains(&path)
        || charts::ASSETS.contains(&path)
        || site::ASSETS.contains(&path)
        || modules::ASSETS.contains(&path)
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
    let mut lines = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok());
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

tokio::task_local! {
    /// The caller of the request being handled, authenticated once by [`remember_identity`].
    pub(super) static IDENTITY: Option<Identity>;
}

/// Who the guard found the caller to be, with the credentials it looked at, so the handler can
/// reuse it while the request still carries the same ones.
#[derive(Clone)]
pub(super) struct Authenticated {
    cookie: Option<HeaderValue>,
    authorization: Option<HeaderValue>,
    identity: Option<Identity>,
}

impl Authenticated {
    fn of(st: &AppState, headers: &HeaderMap) -> Self {
        Authenticated {
            cookie: headers.get(header::COOKIE).cloned(),
            authorization: headers
                .get(header::AUTHORIZATION)
                .cloned(),
            identity: st
                .auth
                .as_ref()
                .and_then(|a| a.authenticate(headers)),
        }
    }

    fn same(&self, headers: &HeaderMap) -> bool {
        self.cookie.as_ref() == headers.get(header::COOKIE)
            && self.authorization.as_ref() == headers.get(header::AUTHORIZATION)
    }
}

/// The innermost layer: authenticates the caller once per request, or takes what the guard found
/// when no layer in between changed the credentials (an OIDC refresh, an account session), and
/// keeps it for every `ctx` the handler builds.
pub(super) async fn remember_identity(
    State(st): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let identity = match req.extensions().get::<Authenticated>() {
        Some(a) if a.same(req.headers()) => a.identity.clone(),
        _ => st
            .auth
            .as_ref()
            .and_then(|a| a.authenticate(req.headers())),
    };
    IDENTITY
        .scope(identity, next.run(req))
        .await
}

/// IP rules and rate limits, before any handler runs. Files of public media sources cost nothing
/// while the client is within its limit, so a page of images never spends it; a request for an
/// id the source does not have (`404`) is counted, so ids cannot be probed without limit.
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
    if let Some(ready) = st
        .auth
        .as_ref()
        .and_then(|a| a.prepare(headers))
    {
        ready.await;
    }
    let peer = parts
        .extensions
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip());
    let forwarded = forwarded_for(headers);
    let ip = client_ip(peer, forwarded.as_deref(), &st.trusted);
    let probe = ops::is_probe(path);
    let unlimited = ops::probe_unlimited(&st.extras, path, ip);
    let public_media = matches!(
        parts.method,
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && media::is_public(&st, path);
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
    let is_write = (matches!(
        parts.method,
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && (COSTLY_GETS.contains(&path) || path.starts_with("/sitemaps/")))
        || parts.method == axum::http::Method::POST
            && (WRITE_PATHS.contains(&path)
                || files::is_upload(path)
                || path.starts_with("/_zeyn/assistant/")
                || path.starts_with("/_zeyn/account/")
                || path
                    .strip_prefix(st.api_prefix.as_str())
                    .is_some_and(|rest| rest.starts_with('/')));
    let limits = if unlimited {
        [(None, "all"), (None, "write")]
    } else {
        [
            (st.rate, "all"),
            (if is_write { st.write_rate } else { None }, "write"),
        ]
    };
    let mut miss = None;
    let mut seen: Option<Authenticated> = None;
    for (limit, scope) in limits {
        let Some(limit) = limit else { continue };
        let ip_key = ip
            .map(|i| i.to_string())
            .unwrap_or_else(|| "unknown".into());
        let key = match limit.key {
            RateKey::Ip => format!("{scope}:ip:{ip_key}"),
            RateKey::User => match seen
                .get_or_insert_with(|| Authenticated::of(&st, headers))
                .identity
                .as_ref()
                .and_then(|i| i.user.clone())
            {
                Some(user) => format!("{scope}:user:{user}"),
                None => format!("{scope}:ip:{ip_key}"),
            },
        };
        let taken = if public_media {
            st.rate_store
                .charge(&key, &limit, 0)
                .await
        } else {
            st.rate_store.take(&key, &limit).await
        };
        if let Err(wait) = taken {
            let ctx = st.ctx(headers);
            let mut response = problem(
                &ctx,
                Failure::App(Error::Denied(Text::key("zeyn.error.rate_limited"))),
            );
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Some(body) = response
                .headers_mut()
                .get_mut(header::CONTENT_TYPE)
            {
                *body = HeaderValue::from_static("application/problem+json");
            }
            let secs = wait.as_secs_f64().ceil().max(1.0) as u64;
            if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, v);
            }
            return response;
        }
        if public_media {
            miss = Some((key, limit));
        }
    }
    let mut parts = parts;
    if let Some(a) = seen {
        parts.extensions.insert(a);
    }
    let response = next
        .run(axum::extract::Request::from_parts(parts, body))
        .await;
    if let Some((key, limit)) = miss
        && response.status() == StatusCode::NOT_FOUND
    {
        let _ = st.rate_store.take(&key, &limit).await;
    }
    response
}

/// One `request` span per request: method, path without the query string, status and duration,
/// plus the call's kind, screen, action, user, tenant and outcome once the handler knows them.
pub(super) async fn trace_request(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
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
    let response = next
        .run(req)
        .instrument(span.clone())
        .await;
    span.record("status", response.status().as_u16());
    span.record("ms", started.elapsed().as_millis() as u64);
    response
}

/// Security headers on every response. The policy is the one built at startup from `App::csp`.
pub(super) async fn security_headers(
    State(st): State<Arc<AppState>>,
    mut response: Response,
) -> Response {
    let h = response.headers_mut();
    h.entry(header::CONTENT_SECURITY_POLICY)
        .or_insert_with(|| st.csp_header.clone());
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response
}
