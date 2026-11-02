//! The request an accounts call runs in: its cookies, the cookies it sets, the client address and
//! the app's rate store. The server opens a scope around every request when accounts are on, so
//! sign in actions of the default UI can set the session cookie although actions only return
//! effects. Outside a request (in tests that call the managers directly) reads find nothing and
//! writes are dropped.

use crate::security::{RateLimit, RateStore};
use std::future::Future;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct Scope {
    pub cookies: String,
    pub out: Mutex<Vec<String>>,
    pub ip: Option<IpAddr>,
    pub secure: bool,
    pub path: String,
    pub prefix: String,
    pub rate: Arc<dyn RateStore>,
}

tokio::task_local! {
    static SCOPE: Arc<Scope>;
}

/// Runs `f` inside `scope`.
pub(crate) async fn run<F: Future>(scope: Arc<Scope>, f: F) -> F::Output {
    SCOPE.scope(scope, f).await
}

fn with<T>(f: impl FnOnce(&Scope) -> T) -> Option<T> {
    SCOPE.try_with(|s| f(s)).ok()
}

/// A cookie of the request, or one this request set already.
pub(crate) fn cookie(name: &str) -> Option<String> {
    with(|s| {
        let set = s.out.lock().ok().and_then(|out| {
            out.iter().rev().find_map(|c| {
                let (k, rest) = c.split_once('=')?;
                (k == name).then(|| rest.split(';').next().unwrap_or("").to_string())
            })
        });
        match set {
            Some(v) if v.is_empty() => None,
            Some(v) => Some(v),
            None => s
                .cookies
                .split(';')
                .filter_map(|p| p.trim().split_once('='))
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string()),
        }
    })
    .flatten()
}

/// Sets a cookie on the response: `HttpOnly`, `SameSite=Lax`, `Secure` on HTTPS. `None` as value
/// deletes it; `None` as age makes a browser session cookie.
pub(crate) fn set_cookie(name: &str, value: Option<&str>, max_age: Option<i64>) {
    with(|s| {
        let secure = if s.secure { "; Secure" } else { "" };
        let cookie = match (value, max_age) {
            (Some(v), Some(age)) => {
                format!("{name}={v}; Path={}; HttpOnly; SameSite=Lax; Max-Age={}{secure}", s.path, age.max(0))
            }
            (Some(v), None) => format!("{name}={v}; Path={}; HttpOnly; SameSite=Lax{secure}", s.path),
            (None, _) => format!("{name}=; Path={}; HttpOnly; SameSite=Lax; Max-Age=0{secure}", s.path),
        };
        if let Ok(mut out) = s.out.lock() {
            out.retain(|c| !c.starts_with(&format!("{name}=")));
            out.push(cookie);
        }
    });
}

/// Takes back a cookie this request set, so the response does not carry it.
pub(crate) fn forget(name: &str) {
    with(|s| {
        if let Ok(mut out) = s.out.lock() {
            out.retain(|c| !c.starts_with(&format!("{name}=")));
        }
    });
}

/// The client's address.
pub(crate) fn ip() -> Option<IpAddr> {
    with(|s| s.ip).flatten()
}

/// The app's base path.
pub(crate) fn prefix() -> String {
    with(|s| s.prefix.clone()).unwrap_or_default()
}

/// Takes one attempt from `key`'s allowance in the app's rate store. Always allowed outside a
/// request.
pub(crate) async fn limit(key: &str, limit: RateLimit) -> Result<(), Duration> {
    let Some(store) = with(|s| s.rate.clone()) else { return Ok(()) };
    store.take(key, &limit).await
}

/// The cookies set by the request.
pub(crate) fn taken(scope: &Scope) -> Vec<String> {
    scope.out.lock().map(|mut o| std::mem::take(&mut *o)).unwrap_or_default()
}
