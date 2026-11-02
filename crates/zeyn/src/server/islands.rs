//! Serving WASM islands: the loader, the files `zeyn build-islands` wrote, and what a page that
//! holds an island needs (the loader in its head and a policy that lets it compile WebAssembly).
//!
//! Pages without an island are untouched: no loader, no extra request, the strict policy.

use super::{AppState, asset};
use crate::render::escape;
use crate::ui::ISLAND_MARKER;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use serde_json::{Map, Value as Json};
use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

const JS: &str = include_str!("../../assets/islands.js");
const LOADER: &str = "/_zeyn/islands.js";
const FILES: &str = "/_zeyn/islands/";

/// The policy of a page that holds an island. It is the default policy plus `'wasm-unsafe-eval'`
/// in `script-src`, which lets the page compile WebAssembly modules. It still refuses `eval`,
/// `new Function` and inline scripts, and modules still come only from the app's own origin.
pub(super) const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";

/// Built islands read from `manifest.json` at startup. Only file names listed there are served.
pub(crate) struct Islands {
    dir: PathBuf,
    files: HashMap<String, &'static str>,
    /// `<meta name="zeyn-islands">` content: island name to `{ js, wasm }` URLs, escaped.
    meta: String,
}

impl Islands {
    /// Reads `dir/manifest.json`. URLs in the page start with `prefix` (the app's public path).
    pub(crate) fn load(dir: &FsPath, prefix: &str) -> Result<Islands, String> {
        let path = dir.join("manifest.json");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}; run `zeyn build-islands`", path.display()))?;
        let manifest: Json = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut files = HashMap::new();
        let mut urls = Map::new();
        for (name, entry) in manifest["islands"].as_object().into_iter().flatten() {
            let (Some(js), Some(wasm)) = (entry["js"].as_str(), entry["wasm"].as_str()) else {
                return Err(format!("{}: island `{name}` needs `js` and `wasm`", path.display()));
            };
            for (file, kind) in [(js, "text/javascript; charset=utf-8"), (wasm, "application/wasm")] {
                if !valid_file(file) {
                    return Err(format!("{}: `{file}` is not a plain file name", path.display()));
                }
                files.insert(file.to_string(), kind);
            }
            urls.insert(
                name.clone(),
                serde_json::json!({ "js": format!("{prefix}{FILES}{js}"), "wasm": format!("{prefix}{FILES}{wasm}") }),
            );
        }
        Ok(Islands { dir: dir.to_path_buf(), files, meta: escape(&Json::Object(urls).to_string()) })
    }
}

/// Letters, digits, `.`, `_` and `-`, no leading dot: never a path.
fn valid_file(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.len() <= 128
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(LOADER, get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/islands/{file}", get(file))
}

/// Paths served without sign in, like the runtime: the loader and the built files.
pub(super) fn is_asset(path: &str) -> bool {
    path == LOADER || path.starts_with(FILES)
}

/// A built file. Names carry a content hash, so they are cached for a year.
async fn file(State(st): State<Arc<AppState>>, Path(name): Path<String>) -> Response {
    let Some((islands, kind)) = st.islands.as_ref().and_then(|i| i.files.get(&name).map(|k| (i, *k))) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(islands.dir.join(&name)).await {
        Ok(bytes) => {
            ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], bytes)
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Whether rendered screen HTML holds an island the app can load. A screen loaded by `nav.js`
/// that does gets `x-zeyn-islands: 1`: a document loaded without the island policy cannot gain
/// it, so `nav.js` then loads the page in full.
pub(super) fn holds(st: &AppState, html: &str) -> bool {
    st.islands.is_some() && html.contains(ISLAND_MARKER)
}

/// A full page. When the screen holds an island, the head gets the loader and the island URLs,
/// and the response the island policy.
pub(super) fn page(st: &AppState, page: String, screen: &str) -> Response {
    let Some(islands) = st.islands.as_ref().filter(|_| holds(st, screen)) else {
        return Html(page).into_response();
    };
    let head = format!(
        r#"<meta name="zeyn-islands" content="{}"><script type="module" src="{}{LOADER}"></script></head>"#,
        islands.meta,
        escape(&st.prefix)
    );
    let mut r = Html(page.replacen("</head>", &head, 1)).into_response();
    r.headers_mut().insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_names_cannot_escape_the_folder() {
        assert!(valid_file("canvas.0123abcd.js"));
        assert!(!valid_file("../secret"));
        assert!(!valid_file(".env"));
        assert!(!valid_file("a/b.js"));
        assert!(!valid_file("a\\b.js"));
        assert!(!valid_file(""));
    }

    #[test]
    fn load_reads_the_manifest_and_refuses_paths() {
        let dir = std::env::temp_dir().join(format!("zeyn-islands-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.json"), r#"{"islands":{"c":{"js":"c.1.js","wasm":"c_bg.1.wasm"}}}"#).unwrap();
        let i = Islands::load(&dir, "/app").unwrap();
        assert_eq!(i.files.get("c_bg.1.wasm"), Some(&"application/wasm"));
        assert!(i.meta.contains("/app/_zeyn/islands/c.1.js"));
        std::fs::write(dir.join("manifest.json"), r#"{"islands":{"c":{"js":"../x.js","wasm":"c.wasm"}}}"#).unwrap();
        assert!(Islands::load(&dir, "").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
