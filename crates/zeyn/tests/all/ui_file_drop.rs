//! FileDrop: a drop zone outside a form that uploads into a store and runs an action for each
//! stored file, after the server has checked the file's shape and type.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{FileRules, FileType, I18n, Identity, MemoryStore};

const DOCS: FileRules = FileRules::new("docs")
    .max_bytes(64 * 1024)
    .max_files(3)
    .accept(&[FileType::Png, FileType::Pdf]);

#[derive(Clone, Default)]
pub struct Added(Arc<Mutex<Vec<File>>>);

impl Service for Added {}

#[action(permission = "docs.add")]
pub async fn add_document(file: File, added: Added) -> Result<()> {
    if file.name == "refuse.png" {
        return Err(Error::Forbidden);
    }
    added.0.lock().unwrap().push(file);
    Ok(())
}

#[screen(route = "/docs", permission = "docs.read")]
pub fn docs() -> Screen {
    Screen::new("Documents")
        .body(Card::new().add(FileDrop::new("Documents <drop>", DOCS).on_upload(add_document)))
}

#[screen(route = "/plain", permission = "docs.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(FileDrop::new("Photo", DOCS).single())
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ID: &str = "0123456789abcdef0123456789abcdef";

fn app(added: Added) -> axum::Router {
    App::new("Drop")
        .i18n(I18n::new("en"))
        .service(added)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(
            Feature::new("docs")
                .files(DOCS, "docs.add", MemoryStore::default())
                .screen(docs)
                .screen(plain),
        )
        .router()
        .unwrap()
}

async fn send(router: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn report(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    input: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let call = serde_json::json!({
        "screen": "docs",
        "params": null,
        "node": "0.0",
        "key": "upload",
        "input": input,
    });
    send(
        router,
        req.body(Body::from(call.to_string()))
            .unwrap(),
    )
    .await
}

fn file(id: &str, name: &str, ty: &str) -> serde_json::Value {
    serde_json::json!({ "id": id, "name": name, "size": 10, "type": ty })
}

const ADD: &str = "docs.read,docs.add";

#[tokio::test]
async fn a_file_drop_renders_an_upload_zone_with_its_store_label_and_modules() {
    let router = app(Added::default());
    let (status, page) = send(
        &router,
        Request::get("/docs")
            .header("x-perms", "docs.read")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(r#"<div class="z-file-drop" data-zeyn-file-drop data-save>"#));
    assert!(page.contains(r#"data-zeyn-upload="docs" data-multiple="true""#));
    assert!(page.contains(r#"data-max-files="3""#));
    assert!(page.contains("Documents &lt;drop&gt;"));
    assert!(!page.contains("Documents <drop>"));
    assert!(page.contains(r#"accept=".png,image/png,.pdf,application/pdf""#));
    for name in ["files.js", "files.css", "net.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    let (_, page) = send(
        &router,
        Request::get("/plain")
            .header("x-perms", "docs.read")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(page.contains(r#"<div class="z-file-drop" data-zeyn-file-drop>"#));
    assert!(page.contains(r#"data-multiple="false""#));
}

#[tokio::test]
async fn a_stored_file_runs_the_upload_action_with_a_cleaned_name() {
    let added = Added::default();
    let router = app(added.clone());
    let (status, body) = report(&router, ADD, true, file(ID, "../scan<1>.png", "image/png")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored = added.0.lock().unwrap().clone();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, ID);
    assert_eq!(stored[0].name, "scan<1>.png", "the path is cut off");
    assert_eq!(stored[0].content_type, "image/png");
}

#[tokio::test]
async fn the_upload_action_needs_csrf_the_permission_and_a_file_of_the_server() {
    let added = Added::default();
    let router = app(added.clone());
    let ok = file(ID, "a.png", "image/png");
    let (status, _) = report(&router, ADD, false, ok.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no CSRF header");
    let (status, _) = report(&router, "docs.read", true, ok).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no permission of the action");
    let bad = [
        file("../../etc/passwd", "a.png", "image/png"),
        file(ID, "a.html", "text/html"),
        file(ID, &"a".repeat(300), "image/png"),
        serde_json::json!({ "id": ID }),
    ];
    for input in bad {
        let (status, _) = report(&router, ADD, true, input.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{input}");
    }
    let (status, _) = report(&router, ADD, true, file(ID, "refuse.png", "image/png")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the action refuses");
    assert!(added.0.lock().unwrap().is_empty());
}
