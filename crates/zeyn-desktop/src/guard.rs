//! The loopback guard: only the app's own window may talk to the local server.
//!
//! Every request must name the exact loopback host and port (this stops DNS rebinding), must
//! not come from another origin, and must carry the per launch token, either in the
//! `zeyn_desktop` cookie that the first navigation sets or in the `x-zeyn-desktop` header.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

/// Name of the cookie that carries the launch token.
pub const COOKIE: &str = "zeyn_desktop";
/// Header that may carry the launch token instead of the cookie.
pub const HEADER: &str = "x-zeyn-desktop";
/// Header that carries the focus key of a second launch.
pub const FOCUS_HEADER: &str = "x-zeyn-focus";
/// The first navigation of the window: `/_desktop/start?t=<token>` sets the cookie.
pub const START: &str = "/_desktop/start";
/// A second launch posts here to bring the first window to the front.
pub const FOCUS: &str = "/_desktop/focus";

/// Why the guard refused a request.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The `Host` header is not exactly `127.0.0.1:<port>`.
    Host,
    /// The `Origin` header names another origin.
    Origin,
    /// Neither the cookie nor the header carries the launch token.
    Token,
}

/// Per launch secrets and the one origin the server answers.
pub struct Guard {
    token: String,
    focus_key: String,
    host: String,
    origin: String,
    on_focus: Box<dyn Fn() + Send + Sync>,
}

impl Guard {
    /// A guard for `127.0.0.1:<port>` with fresh random secrets.
    pub fn new(port: u16, on_focus: Box<dyn Fn() + Send + Sync>) -> std::io::Result<Self> {
        let host = format!("127.0.0.1:{port}");
        Ok(Self { token: secret()?, focus_key: secret()?, origin: format!("http://{host}"), host, on_focus })
    }

    /// The origin of the app, `http://127.0.0.1:<port>`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The URL the window opens first; it trades the token for a cookie.
    pub fn start_url(&self) -> String {
        format!("{}{START}?t={}", self.origin, self.token)
    }

    /// The key a second launch sends to focus this window.
    pub fn focus_key(&self) -> &str {
        &self.focus_key
    }

    /// Checks the host and origin of a request.
    pub fn check_origin(&self, h: &HeaderMap) -> Result<(), Refusal> {
        if h.get(header::HOST).and_then(|v| v.to_str().ok()) != Some(self.host.as_str()) {
            return Err(Refusal::Host);
        }
        match h.get(header::ORIGIN) {
            Some(o) if o.as_bytes() != self.origin.as_bytes() => Err(Refusal::Origin),
            _ => Ok(()),
        }
    }

    /// Checks host, origin and launch token of a request.
    pub fn check(&self, h: &HeaderMap) -> Result<(), Refusal> {
        self.check_origin(h)?;
        let header_token = h.get(HEADER).map(HeaderValue::as_bytes);
        let cookie_token = h
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .find_map(|c| c.trim().strip_prefix(COOKIE).and_then(|r| r.strip_prefix('=')))
            .map(str::as_bytes);
        if header_token.into_iter().chain(cookie_token).any(|t| same(t, self.token.as_bytes())) {
            Ok(())
        } else {
            Err(Refusal::Token)
        }
    }

    fn start(&self, query: Option<&str>) -> Response {
        let given = query.unwrap_or("").split('&').find_map(|p| p.strip_prefix("t=")).unwrap_or("");
        if !same(given.as_bytes(), self.token.as_bytes()) {
            return refuse();
        }
        let cookie = format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict", self.token);
        let mut r = (StatusCode::SEE_OTHER, [(header::LOCATION, "/")]).into_response();
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            r.headers_mut().insert(header::SET_COOKIE, v);
        }
        r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        r
    }
}

/// Middleware that applies the guard in front of the whole app.
pub async fn layer(State(g): State<Arc<Guard>>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == START {
        return match g.check_origin(req.headers()) {
            Ok(()) => g.start(req.uri().query()),
            Err(_) => refuse(),
        };
    }
    if path == FOCUS && req.method() == Method::POST {
        let key = req.headers().get(FOCUS_HEADER).map(HeaderValue::as_bytes).unwrap_or_default();
        if g.check_origin(req.headers()).is_ok() && same(key, g.focus_key.as_bytes()) {
            (g.on_focus)();
            return StatusCode::NO_CONTENT.into_response();
        }
        return refuse();
    }
    match g.check(req.headers()) {
        Ok(()) => next.run(req).await,
        Err(_) => refuse(),
    }
}

fn refuse() -> Response {
    (StatusCode::FORBIDDEN, "This server only answers its own desktop window.").into_response()
}

/// 32 random bytes as 64 hex characters.
fn secret() -> std::io::Result<String> {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Compares in time that depends only on the length.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> Guard {
        Guard::new(4321, Box::new(|| {})).unwrap()
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(*k, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn accepts_cookie_and_header_tokens_from_its_own_origin() {
        let g = guard();
        let cookie = format!("theme=dark; {COOKIE}={}", g.token);
        assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321"), ("cookie", &cookie)])), Ok(()));
        assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321"), ("origin", "http://127.0.0.1:4321"), (HEADER, &g.token)])), Ok(()));
    }

    #[test]
    fn refuses_requests_without_the_token() {
        let g = guard();
        assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321")])), Err(Refusal::Token));
        let wrong = format!("{COOKIE}={}", "0".repeat(64));
        assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321"), ("cookie", &wrong)])), Err(Refusal::Token));
        let prefix = format!("{COOKIE}x={}", g.token);
        assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321"), ("cookie", &prefix)])), Err(Refusal::Token));
    }

    #[test]
    fn refuses_rebound_hosts_and_foreign_origins_even_with_the_token() {
        let g = guard();
        for host in ["evil.example:4321", "localhost:4321", "127.0.0.1:4322", "127.0.0.1"] {
            assert_eq!(g.check(&headers(&[("host", host), (HEADER, &g.token)])), Err(Refusal::Host), "{host}");
        }
        assert_eq!(g.check(&headers(&[(HEADER, &g.token)])), Err(Refusal::Host));
        for origin in ["http://evil.example", "null", "http://localhost:4321", "https://127.0.0.1:4321"] {
            assert_eq!(g.check(&headers(&[("host", "127.0.0.1:4321"), ("origin", origin), (HEADER, &g.token)])), Err(Refusal::Origin), "{origin}");
        }
    }

    #[test]
    fn start_trades_the_right_token_for_a_strict_http_only_cookie() {
        let g = guard();
        assert_eq!(g.start(Some("t=nope")).status(), StatusCode::FORBIDDEN);
        assert_eq!(g.start(None).status(), StatusCode::FORBIDDEN);
        let r = g.start(Some(&format!("t={}", g.token)));
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        let cookie = r.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains(&g.token));
    }

    #[test]
    fn every_launch_gets_new_secrets() {
        let (a, b) = (guard(), guard());
        assert_eq!(a.token.len(), 64);
        assert_ne!(a.token, b.token);
        assert_ne!(a.token, a.focus_key);
        assert!(a.start_url().starts_with("http://127.0.0.1:4321/_desktop/start?t="));
    }
}
