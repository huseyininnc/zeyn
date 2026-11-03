//! Media files served to the `Video` and `Audio` components, with HTTP range requests.
//!
//! An app registers a [`MediaSource`] on a feature with a permission:
//!
//! ```ignore
//! Feature::new("library").media("films", "library.watch", FilmFiles::new(dir))
//! ```
//!
//! The browser asks for `GET /_zeyn/media/{source}/{id}`. The server checks the id's shape, the
//! permission and the feature's guards, then asks the source to open the id. The id is a name the
//! app looks up (in a table or a fixed list); it never names a file path, so a client cannot walk
//! the file system. Files are streamed in small chunks, never read whole into memory.

use crate::action::{Error, Result};
use crate::ctx::{BoxFuture, Ctx};
use std::io::{Cursor, Read, Seek};
use std::path::Path;

/// Longest media id, in bytes.
pub const MAX_MEDIA_ID: usize = 128;

/// Bytes read from a media reader at a time.
pub const MEDIA_CHUNK: usize = 64 * 1024;

/// Longest saved play position, in seconds (about 115 days). Longer values are refused.
pub const MAX_POSITION_SECONDS: f64 = 10_000_000.0;

/// What a media response is read from. Any `Read + Seek` works, such as `std::fs::File`.
pub trait MediaReader: Read + Seek + Send + 'static {}

impl<T: Read + Seek + Send + 'static> MediaReader for T {}

/// An opened media file: its reader, length, content type and optional entity tag.
pub struct Media {
    pub(crate) reader: Box<dyn MediaReader>,
    pub(crate) len: u64,
    pub(crate) content_type: String,
    pub(crate) etag: Option<String>,
}

impl Media {
    /// Media read from `reader`, which must hold exactly `len` bytes.
    pub fn new(reader: impl MediaReader, len: u64, content_type: impl Into<String>) -> Self {
        Media {
            reader: Box::new(reader),
            len,
            content_type: content_type.into(),
            etag: None,
        }
    }

    /// Media held in memory, such as `include_bytes!` data.
    ///
    /// ```
    /// let m = zeyn::Media::bytes(&b"WEBVTT\n"[..], "text/vtt");
    /// assert_eq!(m.len(), 7);
    /// ```
    pub fn bytes<T: AsRef<[u8]> + Send + 'static>(
        data: T,
        content_type: impl Into<String>,
    ) -> Self {
        let len = data.as_ref().len() as u64;
        Media::new(Cursor::new(data), len, content_type)
    }

    /// A file on disk. The content type comes from the extension ([`media_type`]) and the entity
    /// tag from the length and modification time, so browsers can resume and cache it. The path
    /// must come from the app's own lookup, never from the client.
    pub fn file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path)?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(std::io::Error::other("not a file"));
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|m| {
                m.duration_since(std::time::UNIX_EPOCH)
                    .ok()
            })
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        Ok(Media::new(file, meta.len(), media_type(name))
            .etag(format!("{:x}-{modified:x}", meta.len())))
    }

    /// Sets a strong entity tag (without quotes). It must change whenever the bytes change.
    pub fn etag(mut self, tag: impl Into<String>) -> Self {
        let tag: String = tag
            .into()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        self.etag = (!tag.is_empty()).then_some(tag);
        self
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }
}

/// Where an app keeps its media: films, songs, recordings, captions. Registered with
/// [`Feature::media`](crate::Feature::media) under a permission.
///
/// `open` gets an id that has already passed [`valid_media_id`] and the permission check. Look
/// it up in the app's own catalog and return [`Error::NotFound`] for anything unknown.
///
/// Positions are optional: a source that keeps them lets users resume on any device. Without
/// them (the default), the player keeps the position in the browser's local storage.
pub trait MediaSource: Send + Sync + 'static {
    fn open(&self, ctx: Ctx, id: String) -> BoxFuture<Result<Media>>;

    /// Where the signed in user stopped in media `id`, in seconds. `Err(NotFound)` means the
    /// source does not keep positions.
    fn position(&self, _ctx: Ctx, _id: String) -> BoxFuture<Result<Option<f64>>> {
        Box::pin(async { Err(Error::NotFound) })
    }

    /// Saves where the signed in user stopped. `0` means from the start (the media ended).
    fn save_position(&self, _ctx: Ctx, _id: String, _seconds: f64) -> BoxFuture<Result<()>> {
        Box::pin(async { Err(Error::NotFound) })
    }
}

/// A media source registered on a feature.
#[derive(Clone)]
pub struct MediaEntry {
    pub name: &'static str,
    pub permission: &'static str,
    pub source: std::sync::Arc<dyn MediaSource>,
    /// `Some(seconds)` for a source anyone may read ([`Feature::public_media`](crate::Feature::public_media)):
    /// no permission check, and browsers and shared caches keep responses that long.
    pub public_cache: Option<u32>,
}

/// Longest cache time of a public media source, in seconds (one year).
pub const MAX_PUBLIC_MEDIA_CACHE: u32 = 31_536_000;

/// Whether a media id or source name has a safe shape: 1 to 128 ASCII letters, digits, `-`, `_`
/// and `.`, not starting with a dot and without `..`. Ids that fail are answered with `404`
/// before any source sees them.
///
/// ```
/// use zeyn::valid_media_id;
/// assert!(valid_media_id("intro.en.vtt"));
/// assert!(!valid_media_id("../etc/passwd"));
/// assert!(!valid_media_id(".env"));
/// assert!(!valid_media_id("a/b"));
/// ```
pub fn valid_media_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_MEDIA_ID
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Content type for a file name, by its extension; `application/octet-stream` when unknown.
///
/// ```
/// assert_eq!(zeyn::media_type("a.mp4"), "video/mp4");
/// assert_eq!(zeyn::media_type("a.VTT"), "text/vtt; charset=utf-8");
/// ```
pub fn media_type(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "ts" => "video/mp2t",
        "m4s" => "video/iso.segment",
        "mp3" => "audio/mpeg",
        "m4a" | "aac" => "audio/mp4",
        "oga" | "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "weba" => "audio/webm",
        "vtt" => "text/vtt; charset=utf-8",
        "m3u8" => "application/vnd.apple.mpegurl",
        "mpd" => "application/dash+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

/// Whether a content type is safe to serve inline from the app's origin: audio, video, raster
/// images, captions and streaming playlists. Anything else (HTML, SVG, scripts) is sent as a
/// download so it can never run as a page of the app.
pub(crate) fn inline_type(content_type: &str) -> bool {
    let t = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    (t.starts_with("audio/")
        || t.starts_with("video/")
        || (t.starts_with("image/") && t != "image/svg+xml"))
        || matches!(
            t.as_str(),
            "text/vtt"
                | "application/vnd.apple.mpegurl"
                | "application/x-mpegurl"
                | "application/dash+xml"
                | "application/mp4"
        )
}

/// What a `Range` header asks for, for a body of a known length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteRange {
    /// No range, another unit or several ranges: the whole body with `200`.
    Full,
    /// First and last byte, inclusive: `206`.
    Part(u64, u64),
    /// A bytes range that is malformed or starts past the end: `416`.
    Unsatisfiable,
}

impl ByteRange {
    /// Reads a single `bytes=` range: `a-b`, `a-` or the suffix `-n`.
    ///
    /// ```
    /// use zeyn::ByteRange;
    /// assert_eq!(ByteRange::parse(Some("bytes=0-99"), 1000), ByteRange::Part(0, 99));
    /// assert_eq!(ByteRange::parse(Some("bytes=-500"), 1000), ByteRange::Part(500, 999));
    /// assert_eq!(ByteRange::parse(Some("bytes=900-"), 1000), ByteRange::Part(900, 999));
    /// assert_eq!(ByteRange::parse(Some("bytes=0-5000"), 1000), ByteRange::Part(0, 999));
    /// assert_eq!(ByteRange::parse(Some("bytes=1000-"), 1000), ByteRange::Unsatisfiable);
    /// assert_eq!(ByteRange::parse(Some("bytes=5-1"), 1000), ByteRange::Unsatisfiable);
    /// assert_eq!(ByteRange::parse(Some("bytes=0-1,5-6"), 1000), ByteRange::Full);
    /// assert_eq!(ByteRange::parse(None, 1000), ByteRange::Full);
    /// ```
    pub fn parse(header: Option<&str>, len: u64) -> ByteRange {
        let Some(spec) = header.map(str::trim) else {
            return ByteRange::Full;
        };
        let Some(spec) = spec.strip_prefix("bytes=") else {
            return ByteRange::Full;
        };
        if spec.contains(',') {
            return ByteRange::Full;
        }
        let Some((a, b)) = spec.trim().split_once('-') else {
            return ByteRange::Unsatisfiable;
        };
        let num = |s: &str| {
            (!s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
                .then(|| s.parse::<u64>().ok())
                .flatten()
        };
        let (a, b) = (a.trim(), b.trim());
        match (a.is_empty(), num(a), num(b)) {
            (true, _, Some(n)) if n > 0 && len > 0 => {
                ByteRange::Part(len.saturating_sub(n), len - 1)
            }
            (false, Some(start), None) if b.is_empty() && start < len => {
                ByteRange::Part(start, len - 1)
            }
            (false, Some(start), Some(end)) if start <= end && start < len => {
                ByteRange::Part(start, end.min(len - 1))
            }
            _ => ByteRange::Unsatisfiable,
        }
    }
}
