//! Browser modules of content components that only some pages use: share buttons, filter chips,
//! hero slides, reactions and card previews. None is in the page head; components.js imports
//! each one the first time a page holds its component (see `LAZY` there), so pages without them
//! pay nothing.

use super::{AppState, asset};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

const SHARE_JS: &str = include_str!("../../assets/share.js");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 1] = ["/_zeyn/share.js"];

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    let js = "text/javascript; charset=utf-8";
    router.route("/_zeyn/share.js", get(move || async move { asset(SHARE_JS, js) }))
}
