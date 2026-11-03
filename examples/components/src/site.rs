//! What the catalog serves besides the framework: its stylesheet, the small script of its copy
//! buttons, and the sample pictures. Both files are linked from the head of every page, so they
//! load once and keep working while the runtime swaps pages without a reload.

use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{HeaderValue, Response, StatusCode, header};
use axum::routing::get;

const CSS: &str = include_str!("../assets/components.css");
const JS: &str = include_str!("../assets/copy.js");

/// The tags put before `</head>` of every page.
pub const HEAD: &str =
    r#"<link rel="stylesheet" href="/site/components.css"><script type="module" src="/site/copy.js"></script>"#;

/// Largest page the head tags are added to. Pages are far smaller; anything bigger passes as is.
const MAX_PAGE: usize = 4 * 1024 * 1024;

fn asset(body: &'static str, kind: &'static str) -> Response<Body> {
    let mut res = Response::new(Body::from(body));
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
    res
}

async fn picture(Path(file): Path<String>) -> Response<Body> {
    let n = file
        .strip_suffix(".svg")
        .and_then(|n| n.parse::<u32>().ok())
        .filter(|n| (1..=12).contains(n));
    let Some(n) = n else {
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::NOT_FOUND;
        return res;
    };
    let mut res = Response::new(Body::from(crate::data::svg(n)));
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"));
    res
}

/// Adds the catalog's stylesheet and script to an HTML page.
async fn with_head(res: Response<Body>) -> Response<Body> {
    let html = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !html
        || res
            .headers()
            .contains_key(header::CONTENT_ENCODING)
    {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_PAGE).await else {
        parts.status = StatusCode::INTERNAL_SERVER_ERROR;
        return Response::from_parts(parts, Body::empty());
    };
    let page = String::from_utf8_lossy(&bytes);
    let page = match page.find("</head>") {
        Some(at) => format!("{}{HEAD}{}", &page[..at], &page[at..]),
        None => page.into_owned(),
    };
    parts
        .headers
        .remove(header::CONTENT_LENGTH);
    parts.headers.remove(header::ETAG);
    Response::from_parts(parts, Body::from(page))
}

/// The app's router with the catalog's own files and the head tags.
pub fn router(app: Router) -> Router {
    app.route("/site/components.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
        .route("/site/copy.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/site/img/{file}", get(picture))
        .layer(axum::middleware::map_response(with_head))
}
