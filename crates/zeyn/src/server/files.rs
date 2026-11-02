//! File endpoints: uploads into a [`FileStore`](crate::FileStore) and downloads with ranges.
//!
//! `POST /_zeyn/files/{store}` takes the raw file as the body, its name in `X-Zeyn-File-Name`
//! (percent encoded UTF-8), and answers `201` with the stored [`File`](crate::File). The body is
//! read as a stream and handed to the store in chunks of at most [`UPLOAD_CHUNK`] bytes through a
//! channel of four, so memory stays flat for any size. `GET /_zeyn/files/{store}/{id}` serves a
//! file through the media path: ranges, entity tags, `nosniff`, and anything that is not a
//! raster image, audio or video as a download.

use super::{AppState, Call, CallKind, Failure, asset, media};
use crate::action::{Access, Error};
use crate::audit::RecordRef;
use crate::ctx::Ctx;
use crate::files::{
    FileChunks, FileEntry, FileMeta, FileType, SNIFF_BYTES, ScanVerdict, UPLOAD_CHUNK, clean_file_name, new_file_id,
    too_large, valid_file_id,
};
use crate::i18n::Text;
use crate::media::{Media, inline_type};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_core::Stream;
use std::sync::Arc;
use std::time::Duration;

const JS: &str = include_str!("../../assets/files.js");
const CSS: &str = include_str!("../../assets/files.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/files.js", "/_zeyn/files.css"];

/// Longest wait for the next part of an upload before it is dropped.
const IDLE: Duration = Duration::from_secs(30);

/// Links placed in the page head.
pub(super) fn head(base: &str) -> String {
    format!(
        r#"<link rel="stylesheet" href="{base}/_zeyn/files.css"><script type="module" src="{base}/_zeyn/files.js"></script>"#
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/files.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/files.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
        .route("/_zeyn/files/{store}", post(upload).layer(DefaultBodyLimit::disable()))
        .route("/_zeyn/files/{store}/{id}", get(download))
}

/// Whether a path is an upload, for the write limit.
pub(super) fn is_upload(path: &str) -> bool {
    path.strip_prefix("/_zeyn/files/").is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

/// The store behind a request and its feature, after the permission and the guards.
fn entry<'a>(st: &'a AppState, ctx: &Ctx, store: &str) -> Result<(&'a crate::Feature, FileEntry), Failure> {
    let (feature, entry) = st
        .features
        .iter()
        .find_map(|f| f.file_stores().iter().find(|e| e.rules.store() == store).map(|e| (f, e)))
        .ok_or(Failure::App(Error::NotFound))?;
    if !ctx.identity().can(&Access::Permission(entry.permission)) {
        return Err(Failure::App(Error::Forbidden));
    }
    feature.check_guards(ctx)?;
    Ok((feature, entry.clone()))
}

fn status(code: StatusCode, name: &'static str, message: Text) -> Failure {
    Failure::Status(code, name, message)
}

async fn upload(
    State(st): State<Arc<AppState>>,
    Path(store): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let (feature, entry) = entry(&st, &ctx, &store)?;
        let call =
            Call { kind: CallKind::Upload, screen: entry.rules.store(), action: None, via_agent: ctx.via_agent() };
        let scanner = feature.file_scanner().cloned();
        st.intercept(&ctx, &call, || receive(&ctx, entry, scanner, &headers, body)).await
    }
    .await;
    match result {
        Ok(file) => {
            let mut r = (StatusCode::CREATED, axum::Json(file)).into_response();
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Reads the body, checks it and stores it. See the module documentation.
async fn receive(
    ctx: &Ctx,
    entry: FileEntry,
    scanner: Option<Arc<dyn crate::FileScanner>>,
    headers: &HeaderMap,
    body: Body,
) -> Result<crate::File, Failure> {
    let rules = entry.rules;
    let max = rules.limit();
    let big = || status(StatusCode::PAYLOAD_TOO_LARGE, "too_large", too_large(&rules, ctx.locale()));
    let declared =
        headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > max) {
        return Err(big());
    }
    let name = clean_file_name(&decode(headers.get("x-zeyn-file-name").and_then(|v| v.to_str().ok()).unwrap_or("")));
    let mut stream = body.into_data_stream();
    let mut head: Vec<u8> = Vec::with_capacity(SNIFF_BYTES);
    let mut pending: Vec<Bytes> = Vec::new();
    let mut ended = false;
    while head.len() < SNIFF_BYTES {
        match next(&mut stream).await? {
            Some(chunk) => {
                head.extend_from_slice(&chunk[..chunk.len().min(SNIFF_BYTES - head.len())]);
                pending.push(chunk);
            }
            None => {
                ended = true;
                break;
            }
        }
    }
    if head.is_empty() {
        return Err(status(StatusCode::UNPROCESSABLE_ENTITY, "empty_file", Text::key("zeyn.files.empty")));
    }
    let kind = FileType::detect(&name, &head)
        .filter(|t| rules.allows(*t))
        .ok_or_else(|| status(StatusCode::UNSUPPORTED_MEDIA_TYPE, "file_type", Text::key("zeyn.files.type")))?;
    drop(head);
    let meta = FileMeta {
        id: new_file_id(),
        name,
        content_type: kind.mime().to_string(),
        size: 0,
        tenant: ctx.tenant().map(|t| t.id().to_string()),
        owner: ctx.identity().user.clone(),
        created: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    };
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(4);
    let put = tokio::spawn(entry.store.put(meta.clone(), FileChunks(rx)));
    let mut size = 0u64;
    let mut failure: Option<Failure> = None;
    let mut queue = pending.into_iter();
    loop {
        let chunk = match queue.next() {
            Some(c) => c,
            None if ended => break,
            None => match next(&mut stream).await {
                Ok(Some(c)) => c,
                Ok(None) => break,
                Err(f) => {
                    failure = Some(f);
                    break;
                }
            },
        };
        size += chunk.len() as u64;
        if size > max {
            failure = Some(big());
            break;
        }
        let mut at = 0;
        while at < chunk.len() {
            let end = (at + UPLOAD_CHUNK).min(chunk.len());
            if tx.send(Ok(chunk.slice(at..end))).await.is_err() {
                break;
            }
            at = end;
        }
    }
    if failure.is_some() {
        let _ = tx.send(Err(std::io::Error::other("upload stopped"))).await;
    }
    drop(tx);
    let stored = put.await.map_err(|e| Failure::App(Error::Internal(e.to_string())))?;
    if let Some(f) = failure {
        let _ = entry.store.delete(meta.id.clone()).await;
        return Err(f);
    }
    stored.map_err(Failure::App)?;
    if let Some(scanner) = scanner {
        let refuse = || status(StatusCode::UNPROCESSABLE_ENTITY, "file_refused", Text::key("zeyn.files.refused"));
        let verdict = match entry.store.get(meta.id.clone()).await {
            Ok(file) => scanner.scan(file.meta, file.reader).await,
            Err(e) => Err(e),
        };
        match verdict {
            Ok(ScanVerdict::Clean) => {}
            Ok(ScanVerdict::Refused(reason)) => {
                tracing::warn!(store = rules.store(), reason = %reason, "uploaded file refused by the scanner");
                let _ = entry.store.delete(meta.id.clone()).await;
                return Err(refuse());
            }
            Err(e) => {
                tracing::warn!(store = rules.store(), error = ?e, "file scanner failed; the file was refused");
                let _ = entry.store.delete(meta.id.clone()).await;
                return Err(refuse());
            }
        }
    }
    ctx.update_trail(|t| t.record = Some(RecordRef { model: "file", id: Some(meta.id.clone()) }));
    Ok(crate::File { size, ..meta.file() })
}

/// The next part of the body. A client that stops sending for [`IDLE`] or breaks the
/// connection ends the upload.
async fn next<S>(stream: &mut S) -> Result<Option<Bytes>, Failure>
where
    S: Stream<Item = Result<Bytes, axum::Error>> + Unpin,
{
    let read = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx));
    let aborted = || Failure::Status(StatusCode::BAD_REQUEST, "upload_aborted", Text::key("zeyn.files.aborted"));
    match tokio::time::timeout(IDLE, read).await {
        Ok(Some(Ok(chunk))) => Ok(Some(chunk)),
        Ok(None) => Ok(None),
        Ok(Some(Err(_))) | Err(_) => Err(aborted()),
    }
}

/// Decodes the percent encoded file name header; bad sequences stay as they are.
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        if b[i] == b'%' && i + 3 <= b.len() {
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `Content-Disposition` for a stored name: an ASCII fallback in quotes and the UTF-8 name per
/// RFC 6266, so no name can break out of the header.
fn disposition(kind: &str, name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')') { c } else { '_' })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

async fn download(
    State(st): State<Arc<AppState>>,
    Path((store, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let opened = async {
        if !valid_file_id(&id) {
            return Err(Failure::App(Error::NotFound));
        }
        let (_, entry) = entry(&st, &ctx, &store)?;
        let call =
            Call { kind: CallKind::Media, screen: entry.rules.store(), action: None, via_agent: ctx.via_agent() };
        let tenant = ctx.tenant().map(|t| t.id().to_string());
        st.intercept(&ctx, &call, || async {
            let file = st.timed(entry.store.get(id.clone())).await?;
            if file.meta.tenant != tenant || file.meta.id != id {
                return Err(Failure::App(Error::NotFound));
            }
            Ok(file)
        })
        .await
    }
    .await;
    let file = match opened {
        Ok(f) => f,
        Err(f) => return st.error_response(&ctx, f),
    };
    let inline = inline_type(&file.meta.content_type);
    let media = Media::new(file.reader, file.meta.size, file.meta.content_type.clone()).etag(file.meta.id.clone());
    let mut r = media::respond(media, &headers);
    let h = r.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&disposition(if inline { "inline" } else { "attachment" }, &file.meta.name)) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; sandbox"));
    r
}
