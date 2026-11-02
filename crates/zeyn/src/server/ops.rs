//! Runtime assets and page head additions for the enterprise features.

use super::{AppState, asset};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{attr, escape};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

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

/// Texts the runtime shows, in the user's language.
const TEXTS: &[&str] = &[
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
        r#"<meta name="zeyn-texts" content="{texts}"><link rel="stylesheet" href="{base}/_zeyn/enterprise.css"><script type="module" src="{base}/_zeyn/enterprise.js"></script>"#,
        texts = attr(&texts),
        base = escape(&st.prefix),
    )
}
