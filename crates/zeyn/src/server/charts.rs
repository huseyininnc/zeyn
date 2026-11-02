//! Assets for chart keyboard navigation and tooltips, sparklines, bullets and KPI cards.

use super::{AppState, asset};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/charts.js");
const CSS: &str = include_str!("../../assets/charts.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/charts.js", "/_zeyn/charts.css"];

/// Links placed in the page head.
pub(super) fn head(base: &str) -> String {
    format!(
        r#"<link rel="stylesheet" href="{base}/_zeyn/charts.css"><script type="module" src="{base}/_zeyn/charts.js"></script>"#
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/charts.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/charts.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
}
