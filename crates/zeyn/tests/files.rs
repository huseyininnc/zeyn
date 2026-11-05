//! File uploads and downloads: limits, types by content, ids, cancel, scanning, permissions,
//! the form field and its server side checks.

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{
    BoxFuture, Component, FileAccess, FileChunks, FileMeta, FileScanner, FileStore, I18n, Identity,
    MediaReader, Render, ScanVerdict,
};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

const DOCS: FileRules = FileRules::new("docs")
    .max_bytes(64 * 1024)
    .max_files(2)
    .accept(&[FileType::Png, FileType::Pdf, FileType::Csv]);

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn png(len: usize) -> Vec<u8> {
    let mut v = PNG.to_vec();
    v.resize(len, 7);
    v
}

/// Refuses files whose name has `virus` in it, and records what it saw.
#[derive(Clone, Default)]
struct Scanner(Arc<Mutex<Vec<(String, usize)>>>);

impl FileScanner for Scanner {
    fn scan(
        &self,
        meta: FileMeta,
        mut file: Box<dyn MediaReader>,
    ) -> BoxFuture<Result<ScanVerdict>> {
        let mut data = Vec::new();
        let _ = std::io::Read::read_to_end(&mut file, &mut data);
        self.0
            .lock()
            .unwrap()
            .push((meta.name.clone(), data.len()));
        Box::pin(async move {
            Ok(if meta.name.contains("virus") {
                ScanVerdict::Refused("test signature".into())
            } else {
                ScanVerdict::Clean
            })
        })
    }
}

fn closed(ctx: &Ctx) -> GuardResult {
    if ctx
        .identity()
        .permissions
        .contains("closed")
    {
        GuardResult::Deny("Closed".into())
    } else {
        GuardResult::Allow
    }
}

fn app(store: impl FileStore, scanner: Scanner, audit: MemoryAudit) -> Router {
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
                .unwrap_or("")
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user(user).with_permissions(perms))
        })
        .interceptor(AuditLog(audit))
        .feature(
            Feature::new("crm")
                .guard(closed)
                .files(DOCS, "crm.files", store)
                .scan_files(scanner),
        )
        .router()
        .unwrap()
}

fn upload(name: &str, perms: &str) -> axum::http::request::Builder {
    Request::post("/_zeyn/files/docs")
        .header("x-user", "ada")
        .header("x-perms", perms)
        .header("x-zeyn-csrf", TOKEN)
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-file-name", name)
}

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
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

fn json(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap()
}

fn h(headers: &HeaderMap, name: header::HeaderName) -> &str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

fn download(id: &str, perms: &str) -> Request<Body> {
    Request::get(format!("/_zeyn/files/docs/{id}"))
        .header("x-user", "ada")
        .header("x-perms", perms)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn upload_stores_with_a_server_id_and_download_serves_it() {
    let store = MemoryStore::default();
    let audit = MemoryAudit::default();
    let router = app(store.clone(), Scanner::default(), audit.clone());
    let (status, headers, body) = send(
        &router,
        upload("Kedi%20%C3%B6z.png", "crm.files")
            .body(png(5000).into())
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert_eq!(h(&headers, header::CACHE_CONTROL), "no-store");
    let file = json(&body);
    let id = file["id"].as_str().unwrap().to_string();
    assert!(zeyn::valid_file_id(&id));
    assert_eq!(file["name"], "Kedi öz.png");
    assert_eq!(file["size"], 5000);
    assert_eq!(file["type"], "image/png");
    assert_eq!(store.ids(), vec![id.clone()]);
    let entry = audit
        .entries()
        .into_iter()
        .find(|e| e.kind == zeyn::CallKind::Upload)
        .unwrap();
    assert!(entry.ok);
    assert_eq!(entry.screen, "docs");
    assert_eq!(entry.record.as_deref(), Some(id.as_str()));

    let (status, headers, body) = send(&router, download(&id, "crm.files")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, png(5000));
    assert_eq!(h(&headers, header::CONTENT_TYPE), "image/png");
    assert_eq!(h(&headers, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(
        h(&headers, header::CONTENT_DISPOSITION),
        "inline; filename=\"Kedi _z.png\"; filename*=UTF-8''Kedi%20%C3%B6z.png"
    );
    assert!(h(&headers, header::CONTENT_SECURITY_POLICY).contains("sandbox"));

    let ranged = Request::get(format!("/_zeyn/files/docs/{id}"))
        .header("x-user", "ada")
        .header("x-perms", "crm.files")
        .header(header::RANGE, "bytes=0-7")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&router, ranged).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(h(&headers, header::CONTENT_RANGE), "bytes 0-7/5000");
    assert_eq!(body, PNG[..8].to_vec());
}

#[tokio::test]
async fn documents_and_text_are_forced_to_download() {
    let store = MemoryStore::default();
    let router = app(store, Scanner::default(), MemoryAudit::default());
    for (name, data, kind) in [
        (
            "report.pdf",
            b"%PDF-1.7\n1 0 obj\n".to_vec(),
            "application/pdf",
        ),
        (
            "list.csv",
            b"code;name\nA-1;\"Tea\"\n".to_vec(),
            "text/csv; charset=utf-8",
        ),
    ] {
        let (status, _, body) = send(
            &router,
            upload(name, "crm.files")
                .body(data.into())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{name}");
        let id = json(&body)["id"]
            .as_str()
            .unwrap()
            .to_string();
        let (status, headers, _) = send(&router, download(&id, "crm.files")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            h(&headers, header::CONTENT_TYPE),
            "application/octet-stream",
            "{name}"
        );
        assert!(
            h(&headers, header::CONTENT_DISPOSITION).starts_with("attachment; filename="),
            "{name} {kind}"
        );
        assert_eq!(h(&headers, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    }
}

#[tokio::test]
async fn files_over_the_limit_are_refused_by_header_and_by_stream() {
    let store = MemoryStore::default();
    let router = app(store.clone(), Scanner::default(), MemoryAudit::default());
    let big = png(64 * 1024 + 1);
    let (status, _, body) = send(
        &router,
        upload("a.png", "crm.files")
            .body(big.clone().into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json(&body)["error"], "too_large");
    let chunks: Vec<Result<Bytes, std::io::Error>> = big
        .chunks(10_000)
        .map(|c| Ok(Bytes::copy_from_slice(c)))
        .collect();
    let streamed = Body::from_stream(futures_util::stream::iter(chunks));
    let (status, _, _) = send(
        &router,
        upload("a.png", "crm.files")
            .body(streamed)
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(
        store.ids().is_empty(),
        "nothing is kept of a refused upload"
    );
    let (status, _, _) = send(
        &router,
        upload("a.png", "crm.files")
            .body(png(64 * 1024).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "exactly the limit is fine");
}

#[tokio::test]
async fn types_are_checked_by_content_not_by_name() {
    let store = MemoryStore::default();
    let router = app(store.clone(), Scanner::default(), MemoryAudit::default());
    let cases: [(&str, &[u8]); 7] = [
        ("photo.png", b"<html><script>alert(1)</script></html>"),
        ("photo.png", b"%PDF-1.7\n"),
        ("report.pdf", PNG),
        (
            "image.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
        ),
        ("notes.csv", b"  <!DOCTYPE html><body onload=alert(1)>"),
        ("archive.zip", b"PK\x03\x04rest"),
        ("noextension", PNG),
    ];
    for (name, data) in cases {
        let (status, _, body) = send(
            &router,
            upload(name, "crm.files")
                .body(data.to_vec().into())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{name}");
        assert_eq!(json(&body)["error"], "file_type", "{name}");
    }
    let (status, _, body) = send(
        &router,
        upload("empty.png", "crm.files")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert!(store.ids().is_empty());
    let (status, _, body) = send(
        &router,
        upload("photo.png", "crm.files")
            .header(header::CONTENT_TYPE, "text/html")
            .body(png(100).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        json(&body)["type"],
        "image/png",
        "the browser's content type is ignored"
    );
}

#[tokio::test]
async fn names_and_ids_cannot_walk_the_file_system() {
    let dir = std::env::temp_dir().join(format!("zeyn-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = zeyn::DiskStore::new(dir.join("store"));
    let router = app(store.clone(), Scanner::default(), MemoryAudit::default());
    let (status, _, body) = send(
        &router,
        upload("..%2F..%2Fevil.png", "crm.files")
            .body(png(300).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let file = json(&body);
    assert_eq!(file["name"], "evil.png");
    let id = file["id"].as_str().unwrap();
    let mut names: Vec<String> = std::fs::read_dir(dir.join("store"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .into_string()
                .unwrap()
        })
        .collect();
    names.sort();
    assert_eq!(names, vec![format!("{id}.bin"), format!("{id}.json")]);
    assert!(!dir.join("evil.png").exists());
    for bad in [
        "..%2F..%2Fstore%2Fx",
        "..",
        "ABCDEF0123456789ABCDEF0123456789",
        "123",
        &format!("{id}.json"),
    ] {
        let (status, _, _) = send(&router, download(bad, "crm.files")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{bad}");
    }
    assert!(matches!(
        store
            .get("../../etc/passwd".into())
            .await,
        Err(Error::NotFound)
    ));
    assert!(matches!(
        store.delete("../store".into()).await,
        Err(Error::NotFound)
    ));
    let (status, _, body) = send(&router, download(id, "crm.files")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, png(300));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_canceled_upload_leaves_nothing_behind() {
    let dir = std::env::temp_dir().join(format!("zeyn-files-cancel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let disk = zeyn::DiskStore::new(&dir);
    let memory = MemoryStore::default();
    for router in [
        app(disk.clone(), Scanner::default(), MemoryAudit::default()),
        app(memory.clone(), Scanner::default(), MemoryAudit::default()),
    ] {
        let parts: Vec<Result<Bytes, std::io::Error>> = vec![
            Ok(Bytes::from(png(8000))),
            Ok(Bytes::from(vec![1u8; 8000])),
            Err(std::io::Error::other("client left")),
        ];
        let body = Body::from_stream(futures_util::stream::iter(parts));
        let (status, _, body) = send(
            &router,
            upload("a.png", "crm.files")
                .body(body)
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json(&body)["error"], "upload_aborted");
    }
    assert!(memory.ids().is_empty());
    let left: Vec<_> = std::fs::read_dir(&dir)
        .map(|d| d.collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(left.is_empty(), "no .part, .bin or .json file is left");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn the_scanner_sees_every_file_and_can_refuse_one() {
    let store = MemoryStore::default();
    let scanner = Scanner::default();
    let router = app(store.clone(), scanner.clone(), MemoryAudit::default());
    let (status, _, body) = send(
        &router,
        upload("virus.png", "crm.files")
            .body(png(900).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json(&body)["error"], "file_refused");
    assert!(store.ids().is_empty(), "a refused file is deleted");
    let (status, _, _) = send(
        &router,
        upload("clean.png", "crm.files")
            .body(png(900).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        *scanner.0.lock().unwrap(),
        vec![
            ("virus.png".to_string(), 900),
            ("clean.png".to_string(), 900)
        ]
    );
}

#[tokio::test]
async fn uploads_need_csrf_permission_and_guards() {
    let store = MemoryStore::default();
    let router = app(store.clone(), Scanner::default(), MemoryAudit::default());
    let no_csrf = Request::post("/_zeyn/files/docs")
        .header("x-user", "ada")
        .header("x-perms", "crm.files")
        .header("x-zeyn-file-name", "a.png")
        .body(Body::from(png(100)))
        .unwrap();
    assert_eq!(send(&router, no_csrf).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        send(
            &router,
            upload("a.png", "other")
                .body(png(100).into())
                .unwrap()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &router,
            upload("a.png", "crm.files,closed")
                .body(png(100).into())
                .unwrap()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let unknown = upload("a.png", "crm.files")
        .uri("/_zeyn/files/nope")
        .body(png(100).into())
        .unwrap();
    assert_eq!(send(&router, unknown).await.0, StatusCode::NOT_FOUND);
    assert!(store.ids().is_empty());
    let (status, _, body) = send(
        &router,
        upload("a.png", "crm.files")
            .body(png(100).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json(&body)["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        send(&router, download(&id, "other"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, download(&id, "crm.files,closed"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn uploads_count_against_the_write_limit() {
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("ada").with_permissions(["crm.files"])))
        .write_limit(RateLimit::per_ip(1, std::time::Duration::from_secs(60)))
        .feature(Feature::new("crm").files(DOCS, "crm.files", MemoryStore::default()))
        .router()
        .unwrap();
    let first = send(
        &router,
        upload("a.png", "")
            .body(png(100).into())
            .unwrap(),
    )
    .await
    .0;
    let second = send(
        &router,
        upload("b.png", "")
            .body(png(100).into())
            .unwrap(),
    )
    .await
    .0;
    assert_eq!(
        (first, second),
        (StatusCode::CREATED, StatusCode::TOO_MANY_REQUESTS)
    );
}

#[tokio::test]
async fn files_of_another_tenant_are_not_found() {
    let store = MemoryStore::default();
    let router = app(store.clone(), Scanner::default(), MemoryAudit::default());
    let meta = FileMeta {
        id: "00000000000000000000000000000001".into(),
        name: "other.png".into(),
        content_type: "image/png".into(),
        size: 0,
        tenant: Some("acme".into()),
        owner: None,
        created: 0,
    };
    store
        .put(meta, FileChunks::from_parts(vec![png(50)]))
        .await
        .unwrap();
    assert_eq!(
        send(
            &router,
            download("00000000000000000000000000000001", "crm.files")
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

/// A ticket with attachments and one contract.
#[entity(table = "tickets")]
pub struct Ticket {
    #[key]
    pub id: TicketId,
    #[rule(required, max_len = 80)]
    pub title: String,
    pub contract: Option<File>,
    pub attachments: Vec<File>,
}

fn ticket() -> Ticket {
    Ticket {
        id: TicketId(1),
        title: "Printer".into(),
        contract: None,
        attachments: Vec::new(),
    }
}

fn file_json(id: &str, name: &str) -> serde_json::Value {
    serde_json::json!({ "id": id, "name": name, "size": 10, "type": "image/png" })
}

#[test]
fn the_upload_field_renders_its_limits_and_files() {
    let mut t = ticket();
    t.attachments = vec![File {
        id: "0123456789abcdef0123456789abcdef".into(),
        name: "<b>x</b>.png".into(),
        size: 2048,
        content_type: "image/png".into(),
    }];
    let form = Form::of(t)
        .field(Ticket::title)
        .file(Ticket::contract, DOCS)
        .file(Ticket::attachments, DOCS);
    let mut out = String::new();
    form.render(&mut out, &Render::english("t"));
    assert!(out.contains(r#"data-zeyn-upload="docs""#));
    assert!(out.contains(r#"data-max-bytes="65536""#));
    assert!(out.contains(r#"data-max-files="2""#));
    assert!(out.contains(r#"type="file""#) && out.contains(" multiple"));
    assert!(out.contains(r#"accept=".png,image/png,.pdf,application/pdf,.csv,text/csv""#));
    assert!(out.contains(r#"role="status" aria-live="polite""#));
    assert!(out.contains(r#"data-zeyn-href="/_zeyn/files/docs/0123456789abcdef0123456789abcdef""#));
    assert!(
        out.contains("&lt;b&gt;x&lt;/b&gt;.png"),
        "names are escaped"
    );
    assert!(!out.contains("<b>x</b>"));
    assert!(out.contains("PNG, PDF, CSV, up to 64.0 KB each, at most 2 files."));
}

#[test]
fn the_form_reads_files_and_refuses_bad_ids_and_too_many() {
    let form = Form::of(ticket())
        .field(Ticket::title)
        .file(Ticket::contract, DOCS)
        .file(Ticket::attachments, DOCS);
    let id = |n: u8| format!("{:032x}", n);
    let mut values = serde_json::Map::new();
    values.insert("title".into(), "Printer".into());
    values.insert(
        "contract".into(),
        file_json(&id(1), "../c.pdf")
            .to_string()
            .into(),
    );
    values.insert(
        "attachments".into(),
        serde_json::json!([file_json(&id(2), "a.png")])
            .to_string()
            .into(),
    );
    let saved = form
        .read(&values, "en", TimeZone::UTC)
        .unwrap()
        .into_inner();
    assert_eq!(saved.contract.as_ref().unwrap().name, "c.pdf");
    assert_eq!(saved.attachments.len(), 1);

    values.insert(
        "attachments".into(),
        serde_json::json!([file_json("../../etc/passwd", "a.png")])
            .to_string()
            .into(),
    );
    let errors = form
        .read(&values, "en", TimeZone::UTC)
        .unwrap_err();
    assert_eq!(errors.errors[0].field, "attachments");

    let three: Vec<_> = (3..6)
        .map(|n| file_json(&id(n), "a.png"))
        .collect();
    values.insert(
        "attachments".into(),
        serde_json::Value::Array(three)
            .to_string()
            .into(),
    );
    let errors = form
        .read(&values, "en", TimeZone::UTC)
        .unwrap_err();
    assert_eq!(errors.errors[0].field, "attachments");
    assert_eq!(errors.errors[0].rule, zeyn::Rule::Custom);

    values.insert("attachments".into(), "".into());
    values.insert("contract".into(), "".into());
    let saved = form
        .read(&values, "en", TimeZone::UTC)
        .unwrap()
        .into_inner();
    assert!(saved.contract.is_none() && saved.attachments.is_empty());
}

#[test]
fn rules_detect_types_and_clean_names() {
    assert_eq!(
        FileType::detect("a.JPG", &[0xFF, 0xD8, 0xFF, 0xE0]),
        Some(FileType::Jpeg)
    );
    assert_eq!(
        FileType::detect("a.docx", b"PK\x03\x04"),
        Some(FileType::Docx)
    );
    assert_eq!(FileType::detect("a.txt", b"\x00\x01binary"), None);
    assert_eq!(
        FileType::detect("a.txt", "Türkçe metin, kesik \u{e7}".as_bytes()),
        Some(FileType::Text)
    );
    assert_eq!(
        FileType::detect("a.txt", &"ç".as_bytes()[..1]),
        None,
        "a lone cut character is not text"
    );
    assert_eq!(zeyn::clean_file_name("a\u{0}b\nc.png"), "abc.png");
    assert_eq!(
        zeyn::clean_file_name(&"x".repeat(500))
            .chars()
            .count(),
        zeyn::MAX_FILE_NAME
    );
    assert_eq!(
        FileRules::new("x")
            .max_bytes(u64::MAX)
            .limit(),
        zeyn::MAX_UPLOAD_BYTES
    );
}

#[tokio::test]
async fn readers_download_but_only_writers_upload() {
    let store = MemoryStore::default();
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("ada").with_permissions(perms))
        })
        .feature(Feature::new("crm").files(
            DOCS,
            FileAccess::read("crm.files.read").write("crm.files.write"),
            store.clone(),
        ))
        .router()
        .unwrap();
    let (status, _, _) = send(
        &router,
        upload("a.png", "crm.files.read")
            .body(png(100).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(store.ids().is_empty());
    let (status, _, body) = send(
        &router,
        upload("a.png", "crm.files.write")
            .body(png(100).into())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = json(&body)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _, _) = send(&router, download(&id, "crm.files.write")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = send(&router, download(&id, "crm.files.read")).await;
    assert_eq!(status, StatusCode::OK);
}
