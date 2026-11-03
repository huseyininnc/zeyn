//! Media endpoints: files with HTTP range requests, saved play positions, and the player assets.
//!
//! `GET /_zeyn/media/{source}/{id}` streams a file in bounded chunks with `200`, `206`, `304` or
//! `416`. `GET /_zeyn/media/position` reads and `POST /_zeyn/media/position` saves where the
//! signed in user stopped. Every call checks the id's shape, the source's permission and the
//! feature's guards, and runs the interceptors; the POST also checks the CSRF token.

use super::{AppState, Call, CallKind, Chunks, Failure, asset};
use crate::action::{Access, Error};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::media::{
    ByteRange, MAX_POSITION_SECONDS, MEDIA_CHUNK, Media, MediaEntry, MediaReader, inline_type, valid_media_id,
};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;
use serde_json::json;
use std::io::SeekFrom;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/media.js");
const CSS: &str = include_str!("../../assets/media.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/media.js", "/_zeyn/media.css"];

/// Links placed in the page head after the component library's own.
pub(super) fn head(base: &str) -> String {
    format!(
        r#"<link rel="stylesheet" href="{base}/_zeyn/media.css"><script type="module" src="{base}/_zeyn/media.js"></script>"#
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/media.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/media.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
        .route("/_zeyn/media/position", get(position_get).post(position_save))
        .route("/_zeyn/media/{source}/{id}", get(media_file))
}

/// Whether `path` reads a file of a public media source. These requests skip the rate limit:
/// one page loads many images, and many visitors can share one address behind carrier NAT.
pub(super) fn is_public(st: &AppState, path: &str) -> bool {
    path.strip_prefix("/_zeyn/media/").and_then(|rest| rest.split_once('/')).is_some_and(|(source, id)| {
        !id.contains('/')
            && st.features.iter().any(|f| f.media_sources().iter().any(|m| m.name == source && m.public_cache.is_some()))
    })
}

/// The source behind a request, after the id shape, the permission and the guards.
fn entry(st: &AppState, ctx: &Ctx, source: &str, id: &str) -> Result<MediaEntry, Failure> {
    if !valid_media_id(id) {
        return Err(Failure::App(Error::NotFound));
    }
    let (feature, entry) = st
        .features
        .iter()
        .find_map(|f| f.media_sources().iter().find(|m| m.name == source).map(|m| (f, m)))
        .ok_or(Failure::App(Error::NotFound))?;
    if entry.public_cache.is_none() && !ctx.identity().can(&Access::Permission(entry.permission)) {
        return Err(Failure::App(Error::Forbidden));
    }
    feature.check_guards(ctx)?;
    Ok(entry.clone())
}

/// Runs `work` between the interceptors' `before` and `after`, with the server timeout.
async fn intercepted<T>(
    st: &AppState,
    ctx: &Ctx,
    entry: &MediaEntry,
    work: impl Future<Output = crate::Result<T>>,
) -> Result<T, Failure> {
    let call = Call { kind: CallKind::Media, screen: entry.name, action: None, via_agent: ctx.via_agent() };
    st.intercept(ctx, &call, || st.timed(work)).await
}

async fn media_file(
    State(st): State<Arc<AppState>>,
    Path((source, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let opened = async {
        let entry = entry(&st, &ctx, &source, &id)?;
        let media = intercepted(&st, &ctx, &entry, entry.source.open(ctx.clone(), id)).await?;
        Ok::<_, Failure>((media, entry.public_cache))
    }
    .await;
    match opened {
        Ok((media, public)) => {
            let mut r = respond(media, &headers);
            if let Some(v) = public.and_then(|s| HeaderValue::from_str(&format!("public, max-age={s}")).ok()) {
                r.headers_mut().insert(header::CACHE_CONTROL, v);
            }
            r
        }
        Err(f) => st.error_response(&ctx, f),
    }
}

/// The response for an opened media file and the request's `Range`, `If-Range` and
/// `If-None-Match` headers.
pub(super) fn respond(media: Media, headers: &HeaderMap) -> Response {
    let read = |name: header::HeaderName| headers.get(name).and_then(|v| v.to_str().ok());
    let etag = media.etag.as_ref().map(|t| format!("\"{t}\""));
    let len = media.len;
    let mut range = ByteRange::parse(read(header::RANGE), len);
    if read(header::IF_RANGE).is_some_and(|v| etag.as_deref() != Some(v.trim())) {
        range = ByteRange::Full;
    }
    let inline = inline_type(&media.content_type);
    let mut response = Response::new(Body::empty());
    let h = response.headers_mut();
    let content_type = if inline { media.content_type.as_str() } else { "application/octet-stream" };
    if let Ok(v) = HeaderValue::from_str(content_type) {
        h.insert(header::CONTENT_TYPE, v);
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if !inline {
        h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment"));
    }
    if let Some(v) = etag.as_deref().and_then(|t| HeaderValue::from_str(t).ok()) {
        h.insert(header::ETAG, v);
    }
    if let (Some(tag), Some(wanted)) = (etag.as_deref(), read(header::IF_NONE_MATCH)) {
        if wanted.split(',').any(|w| w.trim() == tag || w.trim() == "*") {
            *response.status_mut() = StatusCode::NOT_MODIFIED;
            return response;
        }
    }
    let (start, count) = match range {
        ByteRange::Unsatisfiable => {
            *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes */{len}")) {
                response.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            return response;
        }
        ByteRange::Full => (0, len),
        ByteRange::Part(first, last) => {
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            if let Ok(v) = HeaderValue::from_str(&format!("bytes {first}-{last}/{len}")) {
                response.headers_mut().insert(header::CONTENT_RANGE, v);
            }
            (first, last - first + 1)
        }
    };
    response.headers_mut().insert(header::CONTENT_LENGTH, HeaderValue::from(count));
    *response.body_mut() = stream(media.reader, start, count);
    response
}

/// Reads `count` bytes from `start` in chunks of at most [`MEDIA_CHUNK`], each on the blocking
/// pool, with at most four chunks waiting for the client. A client that goes away stops it.
fn stream(mut reader: Box<dyn MediaReader>, start: u64, count: u64) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        let mut left = count;
        let mut seek = Some(start);
        while left > 0 {
            let n = left.min(MEDIA_CHUNK as u64) as usize;
            let at = seek.take();
            let read = tokio::task::spawn_blocking(move || {
                if let Some(at) = at {
                    reader.seek(SeekFrom::Start(at))?;
                }
                let mut buf = vec![0u8; n];
                reader.read_exact(&mut buf)?;
                Ok::<_, std::io::Error>((reader, buf))
            })
            .await
            .unwrap_or_else(|_| Err(std::io::Error::other("media read stopped")));
            match read {
                Ok((r, buf)) => {
                    reader = r;
                    left -= n as u64;
                    if tx.send(Ok(Bytes::from(buf))).await.is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            }
        }
    });
    Body::from_stream(Chunks(rx))
}

#[derive(Deserialize)]
struct PositionQuery {
    source: String,
    id: String,
}

#[derive(Deserialize)]
struct PositionCall {
    source: String,
    id: String,
    /// Seconds from the start; `0` when the media ended.
    at: f64,
}

/// Where the signed in user stopped: `{"at": seconds or null, "kept": true}`. When nobody is
/// signed in or the source keeps no positions the answer is `{"at": null, "kept": false}`, and the
/// player uses the browser's storage instead. Unknown sources and ids are `404`.
async fn position_get(State(st): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PositionQuery>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let result = async {
        let entry = entry(&st, &ctx, &q.source, &q.id)?;
        if !ctx.identity().is_signed_in() {
            return Ok(json!({ "at": null, "kept": false }));
        }
        match intercepted(&st, &ctx, &entry, entry.source.position(ctx.clone(), q.id)).await {
            Ok(at) => Ok(json!({ "at": at, "kept": true })),
            Err(Failure::App(Error::NotFound)) => Ok(json!({ "at": null, "kept": false })),
            Err(f) => Err(f),
        }
    }
    .await;
    match result {
        Ok(body) => {
            let mut r = axum::Json(body).into_response();
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Saves where the signed in user stopped. `204`; `404` as for reading; `422` for a position
/// that is negative, not a number or longer than [`MAX_POSITION_SECONDS`].
async fn position_save(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<PositionCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let entry = entry(&st, &ctx, &call.source, &call.id)?;
        if !ctx.identity().is_signed_in() {
            return Err(Failure::App(Error::NotFound));
        }
        if !call.at.is_finite() || !(0.0..=MAX_POSITION_SECONDS).contains(&call.at) {
            return Err(Failure::Status(
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation",
                Text::key("zeyn.ui.media.bad_position"),
            ));
        }
        intercepted(&st, &ctx, &entry, entry.source.save_position(ctx.clone(), call.id, call.at)).await
    }
    .await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}
