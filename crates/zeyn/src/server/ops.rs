//! Runtime assets and page head additions for the enterprise features.

use super::{AppState, asset};
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

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
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
        r#"<meta name="zeyn-build" content="{build}"><meta name="zeyn-texts" content="{texts}"><link rel="stylesheet" href="{base}/_zeyn/enterprise.css"><script type="module" src="{base}/_zeyn/enterprise.js"></script>"#,
        texts = attr(&texts),
        build = escape(build_id()),
        base = escape(&st.prefix),
    )
}
