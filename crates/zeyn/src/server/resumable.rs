//! Resumable uploads (feature `resumable`): the core of tus 1.0 with the creation, expiration
//! and termination extensions, under `/_zeyn/uploads/`.
//!
//! `POST /_zeyn/uploads/{store}` with `Upload-Length` and an optional `Upload-Metadata`
//! `filename` opens an upload and answers `201` with its `Location`. `HEAD` on that location
//! gives `Upload-Offset`, `PATCH` with `Content-Type: application/offset+octet-stream` and the
//! matching `Upload-Offset` appends bytes (`409` when the offset is not the server's), and
//! `DELETE` drops it. Bytes are written to a `{id}.part` file as they arrive, so a connection
//! that breaks keeps what reached the server. When the last byte arrives the file goes through
//! [`store_body`], the path of a plain upload, and that `PATCH` answers `200` with the stored
//! [`File`](crate::File) instead of tus' `204`.
//!
//! Every request needs `Tus-Resumable: 1.0.0`, the store's write permission and the feature's
//! guards; an upload is found only by the user and tenant that opened it. Requests that change
//! something need the CSRF header.

use super::files::{
    check_quota, check_unclaimed, entry, next, quota, status, store_body, uploader,
};
use super::{AppState, Failure};
use crate::ResumableUploads;
use crate::action::Error;
use crate::ctx::Ctx;
use crate::files::{UPLOAD_CHUNK, clean_file_name, new_file_id, too_large, valid_file_id};
use crate::i18n::Text;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{head, post};
use base64::Engine;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The protocol version this server speaks.
const TUS: &str = "1.0.0";

/// The open uploads of the app and where their bytes are kept.
pub(super) struct Uploads {
    settings: ResumableUploads,
    open: Mutex<HashMap<String, Arc<Open>>>,
    /// When the folder was last swept for parts nobody knows, in [`Uploads::now_ms`].
    swept: AtomicU64,
    start: tokio::time::Instant,
}

/// One unfinished upload.
struct Open {
    store: String,
    /// Who uploads, for the count of unclaimed uploads.
    uploader: String,
    owner: Option<String>,
    tenant: Option<String>,
    name: String,
    length: u64,
    offset: AtomicU64,
    /// [`Uploads::now_ms`] of the last bytes, or of the creation.
    touched: AtomicU64,
    /// A `PATCH` is writing.
    busy: AtomicBool,
    /// Terminated or expired; whoever sees it last removes the part file.
    gone: AtomicBool,
}

/// Clears the busy flag of an upload when a `PATCH` ends, also when it ends early, and removes
/// the part file when the upload was dropped meanwhile.
struct Writing<'a>(&'a Open, PathBuf);

impl Drop for Writing<'_> {
    fn drop(&mut self) {
        self.0
            .busy
            .store(false, Ordering::SeqCst);
        if self.0.gone.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&self.1);
        }
    }
}

/// Milliseconds since 1970, for `Upload-Expires` only; expiry itself runs on the monotonic
/// clock of [`Uploads::now_ms`], so a wall clock that jumps expires nothing.
fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Uploads {
    pub(super) fn new(settings: ResumableUploads) -> Self {
        Uploads {
            settings,
            open: Mutex::new(HashMap::new()),
            swept: AtomicU64::new(0),
            start: tokio::time::Instant::now(),
        }
    }

    /// Milliseconds since the app started, on the monotonic clock of the runtime.
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn part(&self, id: &str) -> PathBuf {
        self.settings
            .dir
            .join(format!("{id}.part"))
    }

    fn expires_ms(&self) -> u64 {
        self.settings.expires.as_millis() as u64
    }

    fn expired(&self, open: &Open, now: u64) -> bool {
        !open.busy.load(Ordering::SeqCst)
            && now.saturating_sub(open.touched.load(Ordering::SeqCst)) > self.expires_ms()
    }

    /// Drops expired uploads and their parts. At most once per expiry time it also removes
    /// parts the map does not know, left by a restart.
    async fn sweep(&self) {
        let now = self.now_ms();
        let gone: Vec<PathBuf> = match self.open.lock() {
            Ok(mut open) => {
                let ids: Vec<String> = open
                    .iter()
                    .filter(|(_, o)| self.expired(o, now))
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in &ids {
                    if let Some(o) = open.remove(id) {
                        o.gone.store(true, Ordering::SeqCst);
                    }
                }
                ids.iter()
                    .map(|id| self.part(id))
                    .collect()
            }
            Err(_) => Vec::new(),
        };
        let last = self.swept.load(Ordering::Relaxed);
        let folder = now.saturating_sub(last) > self.expires_ms()
            && self
                .swept
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok();
        let dir = self.settings.dir.clone();
        let age = self.settings.expires;
        if gone.is_empty() && !folder {
            return;
        }
        let _ = tokio::task::spawn_blocking(move || {
            for p in gone {
                let _ = std::fs::remove_file(p);
            }
            if folder {
                crate::files::sweep_parts(&dir, age);
            }
        })
        .await;
    }

    /// The upload `id` of `store` when the caller opened it and it has not expired.
    fn find(&self, ctx: &Ctx, store: &str, id: &str) -> Result<Arc<Open>, Failure> {
        let missing = || Failure::App(Error::NotFound);
        if !valid_file_id(id) {
            return Err(missing());
        }
        let mut open = self
            .open
            .lock()
            .map_err(|_| missing())?;
        let found = open
            .get(id)
            .cloned()
            .ok_or_else(missing)?;
        if found.store != store
            || found.owner != ctx.identity().user
            || found.tenant != ctx.tenant().map(|t| t.id().to_string())
        {
            return Err(missing());
        }
        if self.expired(&found, self.now_ms()) {
            open.remove(id);
            found.gone.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(self.part(id));
            return Err(missing());
        }
        Ok(found)
    }

    /// Takes `id` out of the map; the part file goes when no `PATCH` writes to it.
    fn drop_upload(&self, id: &str, open: &Open) {
        if let Ok(mut map) = self.open.lock() {
            map.remove(id);
        }
        open.gone.store(true, Ordering::SeqCst);
        if !open.busy.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(self.part(id));
        }
    }
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/uploads/{store}", post(create))
        .route(
            "/_zeyn/uploads/{store}/{id}",
            head(offset)
                .patch(append)
                .delete(terminate)
                .layer(DefaultBodyLimit::disable()),
        )
}

fn text_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
}

fn number_header(headers: &HeaderMap, name: &str) -> Option<u64> {
    text_header(headers, name).and_then(|v| v.parse().ok())
}

/// `412` unless the client speaks the version of [`TUS`].
fn check_version(headers: &HeaderMap) -> Result<(), Failure> {
    if text_header(headers, "tus-resumable").as_deref() == Some(TUS) {
        return Ok(());
    }
    Err(status(
        StatusCode::PRECONDITION_FAILED,
        "tus_version",
        Text::key("zeyn.files.aborted"),
    ))
}

/// The `filename` of an `Upload-Metadata` header: comma separated `key base64` pairs.
fn file_name(headers: &HeaderMap) -> String {
    let raw = text_header(headers, "upload-metadata").unwrap_or_default();
    let name = raw
        .split(',')
        .filter_map(|pair| pair.trim().split_once(' '))
        .find(|(k, _)| *k == "filename")
        .and_then(|(_, v)| {
            base64::engine::general_purpose::STANDARD
                .decode(v.trim())
                .ok()
        })
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    clean_file_name(&name)
}

/// An HTTP date, such as `Wed, 21 Oct 2026 07:28:00 GMT`.
fn http_date(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let Some(date) = crate::time::Date::from_days_since_epoch(days) else {
        return String::new();
    };
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
        DAYS[usize::from(date.weekday() - 1)],
        date.day(),
        MONTHS[usize::from(date.month() - 1)],
        date.year(),
        rest / 3600,
        rest / 60 % 60,
        rest % 60,
    )
}

/// Adds the protocol headers every answer carries, and the given ones.
fn answer(mut r: Response, extra: &[(&'static str, String)]) -> Response {
    let h = r.headers_mut();
    h.insert(
        HeaderName::from_static("tus-resumable"),
        HeaderValue::from_static(TUS),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    for (name, value) in extra {
        if let Ok(v) = HeaderValue::from_str(value) {
            h.insert(HeaderName::from_static(name), v);
        }
    }
    r
}

fn expires_header(st: &AppState, open: &Open) -> (&'static str, String) {
    (
        "upload-expires",
        http_date(
            wall_ms()
                + (open.touched.load(Ordering::SeqCst) + st.uploads.expires_ms())
                    .saturating_sub(st.uploads.now_ms()),
        ),
    )
}

async fn create(
    State(st): State<Arc<AppState>>,
    Path(store): Path<String>,
    headers: HeaderMap,
    extensions: axum::http::Extensions,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let made = async {
        st.check_csrf(&headers)?;
        check_version(&headers)?;
        let (_, entry) = entry(&st, &ctx, &store, true)?;
        let length = number_header(&headers, "upload-length").ok_or_else(|| {
            status(
                StatusCode::BAD_REQUEST,
                "upload_length",
                Text::key("zeyn.files.length"),
            )
        })?;
        if length == 0 {
            return Err(status(
                StatusCode::UNPROCESSABLE_ENTITY,
                "empty_file",
                Text::key("zeyn.files.empty"),
            ));
        }
        if length > entry.rules.limit() {
            return Err(status(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                too_large(&entry.rules, ctx.locale()),
            ));
        }
        check_quota(&st, &quota(&ctx, &entry)).await?;
        let who = uploader(&st, &ctx, &headers, &extensions);
        check_unclaimed(&entry, &who).await?;
        st.uploads.sweep().await;
        let uploads = &st.uploads;
        let id = new_file_id();
        let open = Arc::new(Open {
            store: store.clone(),
            uploader: who,
            owner: ctx.identity().user.clone(),
            tenant: ctx.tenant().map(|t| t.id().to_string()),
            name: file_name(&headers),
            length,
            offset: AtomicU64::new(0),
            touched: AtomicU64::new(uploads.now_ms()),
            busy: AtomicBool::new(false),
            gone: AtomicBool::new(false),
        });
        {
            let mut map = uploads
                .open
                .lock()
                .map_err(|_| Failure::App(Error::Internal("upload lock".into())))?;
            let mine = map
                .values()
                .filter(|o| o.owner == open.owner && o.tenant == open.tenant)
                .count();
            if map.len() >= uploads.settings.open || mine >= uploads.settings.per_user {
                return Err(status(
                    StatusCode::TOO_MANY_REQUESTS,
                    "uploads_open",
                    Text::key("zeyn.files.open"),
                ));
            }
            map.insert(id.clone(), open.clone());
        }
        let dir = uploads.settings.dir.clone();
        let part = uploads.part(&id);
        let created = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&dir)?;
            std::fs::File::create_new(&part).map(|_| ())
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
        if let Err(e) = created {
            if let Ok(mut map) = uploads.open.lock() {
                map.remove(&id);
            }
            return Err(Failure::App(Error::Internal(format!(
                "resumable upload: {e}"
            ))));
        }
        Ok((id, open))
    }
    .await;
    match made {
        Ok((id, open)) => {
            let location = st.url(&format!("/_zeyn/uploads/{store}/{id}"));
            answer(
                StatusCode::CREATED.into_response(),
                &[
                    ("location", location),
                    expires_header(&st, &open),
                    ("upload-offset", "0".into()),
                ],
            )
        }
        Err(f) => answer(st.error_response(&ctx, f), &[]),
    }
}

async fn offset(
    State(st): State<Arc<AppState>>,
    Path((store, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let found = check_version(&headers)
        .and_then(|()| entry(&st, &ctx, &store, true).map(|_| ()))
        .and_then(|()| st.uploads.find(&ctx, &store, &id));
    match found {
        Ok(open) => answer(
            StatusCode::OK.into_response(),
            &[
                (
                    "upload-offset",
                    open.offset
                        .load(Ordering::SeqCst)
                        .to_string(),
                ),
                ("upload-length", open.length.to_string()),
                expires_header(&st, &open),
            ],
        ),
        Err(f) => answer(
            st.error_response(&ctx, f)
                .into_response(),
            &[],
        ),
    }
}

async fn terminate(
    State(st): State<Arc<AppState>>,
    Path((store, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let found = st
        .check_csrf(&headers)
        .and_then(|()| check_version(&headers))
        .and_then(|()| entry(&st, &ctx, &store, true).map(|_| ()))
        .and_then(|()| st.uploads.find(&ctx, &store, &id));
    match found {
        Ok(open) => {
            st.uploads.drop_upload(&id, &open);
            answer(StatusCode::NO_CONTENT.into_response(), &[])
        }
        Err(f) => answer(st.error_response(&ctx, f), &[]),
    }
}

async fn append(
    State(st): State<Arc<AppState>>,
    Path((store, id)): Path<(String, String)>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    match write(&st, &ctx, &store, &id, &headers, body).await {
        Ok(Written::Part(open)) => answer(
            StatusCode::NO_CONTENT.into_response(),
            &[
                (
                    "upload-offset",
                    open.offset
                        .load(Ordering::SeqCst)
                        .to_string(),
                ),
                expires_header(&st, &open),
            ],
        ),
        Ok(Written::Whole(file, length)) => {
            let r = (StatusCode::OK, axum::Json(file)).into_response();
            answer(r, &[("upload-offset", length.to_string())])
        }
        Err(f) => answer(st.error_response(&ctx, f), &[]),
    }
}

/// What a `PATCH` led to.
enum Written {
    /// More bytes are due.
    Part(Arc<Open>),
    /// The file is complete and stored, with its length.
    Whole(crate::File, u64),
}

/// Appends the body of a `PATCH` and stores the file when it is complete.
async fn write(
    st: &AppState,
    ctx: &Ctx,
    store: &str,
    id: &str,
    headers: &HeaderMap,
    body: Body,
) -> Result<Written, Failure> {
    st.check_csrf(headers)?;
    check_version(headers)?;
    let (_, entry) = entry(st, ctx, store, true)?;
    let open = st.uploads.find(ctx, store, id)?;
    if text_header(headers, "content-type").as_deref() != Some("application/offset+octet-stream") {
        return Err(status(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "content_type",
            Text::key("zeyn.files.type"),
        ));
    }
    let at = number_header(headers, "upload-offset").ok_or_else(|| {
        status(
            StatusCode::BAD_REQUEST,
            "upload_offset",
            Text::key("zeyn.files.offset"),
        )
    })?;
    if open.busy.swap(true, Ordering::SeqCst) {
        return Err(status(
            StatusCode::LOCKED,
            "upload_busy",
            Text::key("zeyn.files.busy"),
        ));
    }
    let part = st.uploads.part(id);
    let writing = Writing(&open, part.clone());
    if at != open.offset.load(Ordering::SeqCst) {
        return Err(status(
            StatusCode::CONFLICT,
            "upload_offset",
            Text::key("zeyn.files.offset"),
        ));
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(4);
    let target = open.clone();
    let path = part.clone();
    let writer = tokio::task::spawn_blocking(move || {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)?;
        while let Some(chunk) = rx.blocking_recv() {
            file.write_all(&chunk)?;
            target
                .offset
                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
        }
        file.sync_data()
    });
    let mut stream = body.into_data_stream();
    let mut left = open.length - at;
    let mut failure = None;
    loop {
        match next(&mut stream).await {
            Ok(Some(chunk)) => {
                if chunk.len() as u64 > left {
                    failure = Some(status(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "too_large",
                        too_large(&entry.rules, ctx.locale()),
                    ));
                    break;
                }
                left -= chunk.len() as u64;
                if tx.send(chunk).await.is_err() {
                    break;
                }
                open.touched
                    .store(st.uploads.now_ms(), Ordering::SeqCst);
            }
            Ok(None) => break,
            Err(f) => {
                failure = Some(f);
                break;
            }
        }
    }
    drop(tx);
    let wrote = writer
        .await
        .map_err(|e| Failure::App(Error::Internal(e.to_string())))?;
    if let Err(e) = wrote {
        return Err(Failure::App(Error::Internal(format!(
            "resumable upload: {e}"
        ))));
    }
    if let Some(f) = failure {
        if matches!(f, Failure::Status(StatusCode::PAYLOAD_TOO_LARGE, ..)) {
            st.uploads.drop_upload(id, &open);
        }
        return Err(f);
    }
    if open.gone.load(Ordering::SeqCst) {
        return Err(Failure::App(Error::NotFound));
    }
    if open.offset.load(Ordering::SeqCst) < open.length {
        drop(writing);
        return Ok(Written::Part(open));
    }
    if let Ok(mut map) = st.uploads.open.lock() {
        map.remove(id);
    }
    open.gone.store(true, Ordering::SeqCst);
    let stored = store_body(
        st,
        ctx,
        store,
        &open.uploader,
        open.name.clone(),
        Some(open.length),
        file_body(part.clone()),
    )
    .await;
    drop(writing);
    let _ = std::fs::remove_file(&part);
    stored.map(|file| Written::Whole(file, open.length))
}

/// The bytes of a part file as a request body, read on the blocking pool in chunks of
/// [`UPLOAD_CHUNK`] with at most four waiting.
fn file_body(path: PathBuf) -> Body {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::io::Result<Bytes>>(4);
    tokio::task::spawn_blocking(move || {
        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                let _ = tx.blocking_send(Err(e));
                return;
            }
        };
        loop {
            let mut buf = vec![0u8; UPLOAD_CHUNK];
            match file.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => {
                    buf.truncate(n);
                    if tx
                        .blocking_send(Ok(Bytes::from(buf)))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e));
                    return;
                }
            }
        }
    });
    Body::from_stream(futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }))
}
