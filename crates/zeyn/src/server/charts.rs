//! Assets for chart keyboard navigation and tooltips, sparklines, bullets and KPI cards.

use super::{AppState, asset};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/charts.js");
const CSS: &str = include_str!("../../assets/charts.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/charts.js", "/_zeyn/charts.css"];

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/_zeyn/charts.js",
            get(|| async { asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/charts.css",
            get(|| async { asset(CSS, "text/css; charset=utf-8") }),
        )
}
