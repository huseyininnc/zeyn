//! Browser modules of content components that only some pages use: share buttons, filter chips,
//! hero slides, reactions and card previews. None is in the page head; components.js imports
//! each one the first time a page holds its component (see `LAZY` there), so pages without them
//! pay nothing.

use super::{AppState, asset};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

/// The modules by path.
const MODULES: [(&str, &str); 2] = [
    ("/_zeyn/share.js", include_str!("../../assets/share.js")),
    ("/_zeyn/chips.js", include_str!("../../assets/chips.js")),
];

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; MODULES.len()] = {
    let mut paths = [""; MODULES.len()];
    let mut i = 0;
    while i < MODULES.len() {
        paths[i] = MODULES[i].0;
        i += 1;
    }
    paths
};

pub(super) fn routes(mut router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    for (path, body) in MODULES {
        router = router.route(path, get(move || async move { asset(body, "text/javascript; charset=utf-8") }));
    }
    router
}
