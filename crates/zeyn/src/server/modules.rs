//! Browser modules of content components that only some pages use: share buttons, filter chips,
//! hero slides, reactions, card previews and the client preference store (`state.js`). None is in the page head; components.js imports
//! each one the first time a page holds its component (see `LAZY` there), so pages without them
//! pay nothing.

use super::{AppState, asset};
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::sync::Arc;

/// The modules by path.
const MODULES: [(&str, &str); 6] = [
    ("/_zeyn/share.js", include_str!("../../assets/share.js")),
    ("/_zeyn/chips.js", include_str!("../../assets/chips.js")),
    ("/_zeyn/preview.js", include_str!("../../assets/preview.js")),
    (
        "/_zeyn/reactions.js",
        include_str!("../../assets/reactions.js"),
    ),
    ("/_zeyn/hero.js", include_str!("../../assets/hero.js")),
    ("/_zeyn/state.js", include_str!("../../assets/state.js")),
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
        router = router.route(
            path,
            get(move || async move { asset(body, "text/javascript; charset=utf-8") }),
        );
    }
    router.route("/_zeyn/prefs", get(prefs))
}

/// Whether the user consented to keeping preference values beyond the session, by the app's
/// hook (`ClientPrefs::allowed_when`); `false` without one. Never cached, so a public page cache
/// never shares one user's answer.
async fn prefs(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let allowed = st
        .services
        .get::<crate::prefs::ClientPrefs>()
        .and_then(|p| p.allowed)
        .is_some_and(|hook| hook(&headers));
    let mut r = axum::Json(serde_json::json!({ "preferences": allowed })).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}
