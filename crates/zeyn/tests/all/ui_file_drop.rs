//! FileDrop: a drop zone outside a form that uploads into a store and runs an action for each
//! stored file, after the server has checked the file's shape and type.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{FileChunks, FileMeta, FileRules, FileStore, FileType, I18n, Identity, MemoryStore};

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

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn app(added: Added) -> axum::Router {
    app_with(added, MemoryStore::default())
}

fn app_with(added: Added, store: MemoryStore) -> axum::Router {
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
            let user = h
                .get("x-user")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("u")
                .to_string();
            Some(Identity::user(user).with_permissions(perms))
        })
        .feature(
            Feature::new("docs")
                .files(DOCS, "docs.add", store)
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

/// Uploads a PNG into the drop zone's store as `user` and answers its id.
async fn stored(router: &axum::Router, user: &str) -> String {
    let (status, body) = send(
        router,
        Request::post("/_zeyn/files/docs")
            .header("x-user", user)
            .header("x-perms", ADD)
            .header("x-zeyn-csrf", TOKEN)
            .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
            .header("x-zeyn-file-name", "a.png")
            .body(Body::from(PNG))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn report(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    input: serde_json::Value,
) -> (StatusCode, String) {
    report_as(router, "u", perms, csrf, input).await
}

async fn report_as(
    router: &axum::Router,
    user: &str,
    perms: &str,
    csrf: bool,
    input: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post("/_zeyn/data")
        .header("x-user", user)
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
    let id = stored(&router, "u").await;
    let (status, body) = report(&router, ADD, true, file(&id, "../scan<1>.png", "image/png")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let stored = added.0.lock().unwrap().clone();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, id);
    assert_eq!(stored[0].name, "scan<1>.png", "the path is cut off");
    assert_eq!(stored[0].content_type, "image/png");
}

#[tokio::test]
async fn the_upload_action_needs_csrf_the_permission_and_a_file_of_the_server() {
    let added = Added::default();
    let router = app(added.clone());
    let id = stored(&router, "u").await;
    let ok = file(&id, "a.png", "image/png");
    let (status, _) = report(&router, ADD, false, ok.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no CSRF header");
    let (status, _) = report(&router, "docs.read", true, ok).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no permission of the action");
    let bad = [
        file("../../etc/passwd", "a.png", "image/png"),
        file(&id, "a.html", "text/html"),
        file(&id, &"a".repeat(300), "image/png"),
        serde_json::json!({ "id": id }),
    ];
    for input in bad {
        let (status, _) = report(&router, ADD, true, input.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{input}");
    }
    let (status, _) = report(&router, ADD, true, file(&id, "refuse.png", "image/png")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the action refuses");
    assert!(added.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_upload_action_refuses_a_file_another_user_or_tenant_uploaded() {
    let added = Added::default();
    let store = MemoryStore::default();
    let router = app_with(added.clone(), store.clone());
    let theirs = stored(&router, "bob").await;
    let (status, _) = report_as(&router, "u", ADD, true, file(&theirs, "a.png", "image/png")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a file of another user");
    store
        .put(
            FileMeta {
                id: ID.into(),
                name: "a.png".into(),
                content_type: "image/png".into(),
                size: 0,
                tenant: Some("acme".into()),
                owner: Some("u".into()),
                created: 0,
            },
            FileChunks::from_parts(vec![PNG.to_vec()]),
        )
        .await
        .unwrap();
    let (status, _) = report_as(&router, "u", ADD, true, file(ID, "a.png", "image/png")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a file of another tenant");
    let (status, _) = report_as(
        &router,
        "u",
        ADD,
        true,
        file("ffffffffffffffffffffffffffffffff", "a.png", "image/png"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an id the store does not hold"
    );
    assert!(added.0.lock().unwrap().is_empty());
    let (status, body) = report_as(
        &router,
        "bob",
        ADD,
        true,
        file(&theirs, "a.png", "image/png"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(added.0.lock().unwrap().len(), 1);
}
