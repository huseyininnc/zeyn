//! Resumable uploads (feature `resumable`): the tus subset, offsets, broken connections,
//! expiry, caps, access and the checks of the store at the end.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use base64::Engine;
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity, ResumableUploads};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

const DOCS: FileRules = FileRules::new("docs")
    .max_bytes(64 * 1024)
    .accept(&[FileType::Png, FileType::Pdf]);

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn png(len: usize) -> Vec<u8> {
    let mut v = PNG.to_vec();
    v.resize(len, 7);
    v
}

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zeyn-resumable-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn app(settings: ResumableUploads, store: MemoryStore) -> Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let user = h
                .get("x-user")?
                .to_str()
                .ok()?
                .to_string();
            let perms: Vec<String> = h
                .get("x-perms")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("crm.files")
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user(user).with_permissions(perms))
        })
        .resumable_uploads(settings)
        .feature(Feature::new("crm").files(DOCS, "crm.files", store))
        .router()
        .unwrap()
}

fn req(method: &str, uri: &str, user: &str) -> axum::http::request::Builder {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-user", user)
        .header("tus-resumable", "1.0.0")
        .header("x-zeyn-csrf", TOKEN)
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
}

async fn send(router: &Router, r: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let res = router.clone().oneshot(r).await.unwrap();
    let (parts, body) = res.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}

fn h(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

async fn create(router: &Router, user: &str, length: usize) -> (StatusCode, String) {
    let name = base64::engine::general_purpose::STANDARD.encode("scan.png");
    let (status, headers, _) = send(
        router,
        req("POST", "/_zeyn/uploads/docs", user)
            .header("upload-length", length.to_string())
            .header("upload-metadata", format!("filename {name}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    (status, h(&headers, "location"))
}

async fn patch(
    router: &Router,
    at: &str,
    user: &str,
    offset: usize,
    body: Body,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    send(
        router,
        req("PATCH", at, user)
            .header("upload-offset", offset.to_string())
            .header(header::CONTENT_TYPE, "application/offset+octet-stream")
            .body(body)
            .unwrap(),
    )
    .await
}

async fn offset(router: &Router, at: &str, user: &str) -> (StatusCode, String) {
    let (status, headers, _) = send(
        router,
        req("HEAD", at, user)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    (status, h(&headers, "upload-offset"))
}

fn parts(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|d| d.count())
        .unwrap_or(0)
}

#[tokio::test]
async fn an_upload_resumes_from_the_server_offset_and_ends_in_the_store() {
    let dir = folder("flow");
    let store = MemoryStore::default();
    let router = app(ResumableUploads::new(&dir), store.clone());
    let data = png(20_000);
    let (status, at) = create(&router, "ada", data.len()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(at.starts_with("/_zeyn/uploads/docs/"), "{at}");
    assert_eq!(
        offset(&router, &at, "ada").await,
        (StatusCode::OK, "0".into())
    );
    let (status, headers, _) =
        patch(&router, &at, "ada", 0, Body::from(data[..8000].to_vec())).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(h(&headers, "upload-offset"), "8000");
    assert_eq!(h(&headers, "tus-resumable"), "1.0.0");
    assert!(h(&headers, "upload-expires").ends_with(" GMT"));
    let (status, _, _) = patch(&router, &at, "ada", 0, Body::from(data[..10].to_vec())).await;
    assert_eq!(status, StatusCode::CONFLICT, "a stale offset is refused");
    assert_eq!(offset(&router, &at, "ada").await.1, "8000");
    let broken: Vec<Result<Bytes, std::io::Error>> = vec![
        Ok(Bytes::copy_from_slice(&data[8000..12_000])),
        Err(std::io::Error::other("network lost")),
    ];
    let (status, _, _) = patch(
        &router,
        &at,
        "ada",
        8000,
        Body::from_stream(futures_util::stream::iter(broken)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        offset(&router, &at, "ada").await.1,
        "12000",
        "bytes that arrived before the break are kept"
    );
    let (status, headers, body) = patch(
        &router,
        &at,
        "ada",
        12_000,
        Body::from(data[12_000..].to_vec()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(h(&headers, "upload-offset"), "20000");
    let file: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(file["name"], "scan.png");
    assert_eq!(file["size"], 20_000);
    assert_eq!(file["type"], "image/png");
    assert_eq!(store.ids(), vec![file["id"].as_str().unwrap().to_string()]);
    assert_eq!(parts(&dir), 0, "the part file is gone");
    assert_eq!(offset(&router, &at, "ada").await.0, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs on a paused clock: expiry uses the runtime's monotonic time, so the test steps over it
/// instead of racing it on a slow machine.
#[tokio::test(start_paused = true)]
async fn expired_uploads_are_removed_with_their_parts() {
    let dir = folder("expiry");
    let router = app(
        ResumableUploads::new(&dir).expires(Duration::from_secs(3600)),
        MemoryStore::default(),
    );
    let (_, first) = create(&router, "ada", 1000).await;
    let (_, second) = create(&router, "ada", 1000).await;
    patch(&router, &first, "ada", 0, Body::from(png(100))).await;
    assert_eq!(parts(&dir), 2);
    tokio::time::advance(Duration::from_secs(3599)).await;
    assert_eq!(
        offset(&router, &second, "ada").await.1,
        "0",
        "still open before the expiry"
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    assert_eq!(
        offset(&router, &first, "ada").await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _, _) = patch(&router, &first, "ada", 100, Body::from(vec![1u8; 10])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = create(&router, "bob", 1000).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        offset(&router, &second, "ada").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(parts(&dir), 1, "only the new upload has a part file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn open_uploads_are_capped_per_user_and_per_app() {
    let dir = folder("caps");
    let router = app(
        ResumableUploads::new(&dir)
            .max_per_user(2)
            .max_open(3),
        MemoryStore::default(),
    );
    assert_eq!(create(&router, "ada", 100).await.0, StatusCode::CREATED);
    let (_, kept) = create(&router, "ada", 100).await;
    assert_eq!(
        create(&router, "ada", 100).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(create(&router, "bob", 100).await.0, StatusCode::CREATED);
    assert_eq!(
        create(&router, "cyd", 100).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "the app cap"
    );
    let (status, _, _) = send(
        &router,
        req("DELETE", &kept, "ada")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        create(&router, "cyd", 100).await.0,
        StatusCode::CREATED,
        "termination frees a slot"
    );
    assert_eq!(parts(&dir), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn uploads_need_csrf_permission_the_protocol_and_their_owner() {
    let dir = folder("access");
    let router = app(ResumableUploads::new(&dir), MemoryStore::default());
    let (status, _, _) = send(
        &router,
        Request::post("/_zeyn/uploads/docs")
            .header("x-user", "ada")
            .header("tus-resumable", "1.0.0")
            .header("upload-length", "100")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no CSRF header");
    let (status, _, _) = send(
        &router,
        req("POST", "/_zeyn/uploads/docs", "ada")
            .header("x-perms", "other")
            .header("upload-length", "100")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no permission");
    let (status, _, _) = send(
        &router,
        Request::post("/_zeyn/uploads/docs")
            .header("x-user", "ada")
            .header("x-zeyn-csrf", TOKEN)
            .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
            .header("upload-length", "100")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "no Tus-Resumable");
    assert_eq!(
        create(&router, "ada", 64 * 1024 + 1)
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        create(&router, "ada", 0).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, at) = create(&router, "ada", 100).await;
    assert_eq!(
        offset(&router, &at, "bob").await.0,
        StatusCode::NOT_FOUND,
        "another user"
    );
    let (status, _, _) = patch(&router, &at, "bob", 0, Body::from(png(100))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = send(
        &router,
        Request::patch(&at)
            .header("x-user", "ada")
            .header("tus-resumable", "1.0.0")
            .header("upload-offset", "0")
            .header(header::CONTENT_TYPE, "application/offset+octet-stream")
            .body(Body::from(png(100)))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "PATCH without CSRF");
    let (status, _, _) = send(
        &router,
        req("PATCH", &at, "ada")
            .header("upload-offset", "0")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(png(100)))
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "wrong content type"
    );
    let (status, _, _) = patch(&router, &at, "ada", 0, Body::from(png(101))).await;
    assert_eq!(
        status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "more than Upload-Length"
    );
    assert_eq!(
        offset(&router, &at, "ada").await.0,
        StatusCode::NOT_FOUND,
        "and it is dropped"
    );
    let bad = "/_zeyn/uploads/docs/..%2F..%2Fetc";
    assert_eq!(offset(&router, bad, "ada").await.0, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_finished_file_gets_the_checks_of_a_plain_upload() {
    let dir = folder("checks");
    let store = MemoryStore::default();
    let router = app(ResumableUploads::new(&dir), store.clone());
    let text = b"<html><script>alert(1)</script></html>".to_vec();
    let (_, at) = create(&router, "ada", text.len()).await;
    let (status, _, _) = patch(&router, &at, "ada", 0, Body::from(text)).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(store.ids().is_empty());
    assert_eq!(parts(&dir), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn creating_counts_against_the_write_limit_and_its_chunks_do_not() {
    let dir = folder("limit");
    let router = App::new("Test")
        .auth(|_: &HeaderMap| Some(Identity::user("ada").with_permissions(["crm.files"])))
        .write_limit(RateLimit::per_ip(1, Duration::from_secs(60)))
        .resumable_uploads(ResumableUploads::new(&dir))
        .feature(Feature::new("crm").files(DOCS, "crm.files", MemoryStore::default()))
        .router()
        .unwrap();
    let (status, at) = create(&router, "ada", 300).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        create(&router, "ada", 100).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let data = png(300);
    for (from, to, expected) in [
        (0, 100, StatusCode::NO_CONTENT),
        (100, 200, StatusCode::NO_CONTENT),
        (200, 300, StatusCode::OK),
    ] {
        let body = Body::from(data[from..to].to_vec());
        assert_eq!(
            patch(&router, &at, "ada", from, body)
                .await
                .0,
            expected
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
