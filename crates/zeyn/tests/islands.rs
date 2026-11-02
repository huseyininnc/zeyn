use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{Island, Remote};

#[derive(Serialize, Deserialize)]
pub struct Count {
    pub value: i64,
}

pub struct Double;

impl Remote for Double {
    const NAME: &'static str = "double";
    type Input = Count;
    type Output = Count;
}

#[action(permission = "count.write")]
pub async fn double(c: Count) -> Result<Count> {
    Ok(Count { value: c.value * 2 })
}

#[screen(route = "/island", permission = "count.read")]
pub fn with_island() -> Screen {
    Screen::new("Island").body(
        Island::new("counter")
            .props(&Count { value: 1 })
            .fallback(Alert::info("Needs JavaScript"))
            .action::<Double, _>(double),
    )
}

#[screen(route = "/plain", permission = "count.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Alert::info("data-zeyn-island=\"counter\" typed by a user"))
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn router(permissions: &'static [&'static str]) -> Router {
    let dir = std::env::temp_dir().join(format!("zeyn-island-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{"islands":{"counter":{"js":"counter.abc.js","wasm":"counter_bg.def.wasm"}}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("counter_bg.def.wasm"), b"\0asm\x01\0\0\0").unwrap();
    std::fs::write(dir.join("secret.txt"), "no").unwrap();
    App::new("Test")
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user").then(|| Identity::user("u").with_permissions(permissions.iter().copied()))
        })
        .feature(Feature::new("count").screen(with_island).screen(plain))
        .islands(dir)
        .router()
        .unwrap()
}

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, String::from_utf8_lossy(&res.into_body().collect().await.unwrap().to_bytes()).into_owned())
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).header("x-user", "1").body(Body::empty()).unwrap()
}

fn call(key: &str, csrf: bool) -> Request<Body> {
    let body = serde_json::json!({ "screen": "with_island", "params": null, "node": "0", "key": key, "input": { "value": 21 } });
    let mut r = Request::post("/_zeyn/data")
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        r = r.header("x-zeyn-csrf", TOKEN);
    }
    r.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn only_pages_with_an_island_get_the_loader_and_wasm_policy() {
    let app = router(&["count.read"]);
    let (status, headers, body) = send(&app, get("/island")).await;
    assert_eq!(status, StatusCode::OK);
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.contains("script-src 'self' 'wasm-unsafe-eval';"), "{csp}");
    assert!(body.contains(r#"<meta name="zeyn-islands""#) && body.contains("/_zeyn/islands.js"));
    assert!(body.contains("Needs JavaScript"));

    let (_, headers, body) = send(&app, get("/plain")).await;
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(!csp.contains("wasm"), "{csp}");
    assert!(!body.contains("zeyn-islands") && !body.contains("islands.js"));

    let partial =
        Request::get("/island").header("x-user", "1").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    assert_eq!(send(&app, partial).await.1["x-zeyn-islands"], "1");
    let partial =
        Request::get("/plain").header("x-user", "1").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    assert!(!send(&app, partial).await.1.contains_key("x-zeyn-islands"));
}

#[tokio::test]
async fn built_files_are_served_by_manifest_name_only_and_cached() {
    let app = router(&[]);
    let (status, headers, _) =
        send(&app, Request::get("/_zeyn/islands/counter_bg.def.wasm").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/wasm");
    assert!(headers[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));
    for path in ["/_zeyn/islands/secret.txt", "/_zeyn/islands/manifest.json", "/_zeyn/islands/..%2Fsecret.txt"] {
        assert_eq!(
            send(&app, Request::get(path).body(Body::empty()).unwrap()).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    assert_eq!(send(&app, Request::get("/_zeyn/islands.js").body(Body::empty()).unwrap()).await.0, StatusCode::OK);
}

#[tokio::test]
async fn island_calls_go_through_csrf_and_the_actions_permission() {
    let app = router(&["count.read", "count.write"]);
    let (status, _, body) = send(&app, call("double", true)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, r#"{"value":42}"#);
    assert_eq!(send(&app, call("double", false)).await.0, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, call("unbound", true)).await.0, StatusCode::NOT_FOUND);
    let reader = router(&["count.read"]);
    assert_eq!(send(&reader, call("double", true)).await.0, StatusCode::FORBIDDEN);
}
