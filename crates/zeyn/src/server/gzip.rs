//! Gzip for whole dynamic responses with a reused compressor.
//!
//! A deflate compressor holds about 300 KB of tables; making one per response (what the streaming
//! layer does) costs that allocation and its zeroing on every page, partial screen and grid
//! block, as much as rendering them. Bodies whose size is known (every rendered page and JSON
//! answer) are compressed here in one pass with a compressor kept per worker thread and reset
//! between responses. Streamed bodies (exports) still go through the streaming layer.

use axum::body::{Body, Bytes, HttpBody};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use flate2::{Compress, Compression, Crc, FlushCompress, Status};
use std::cell::RefCell;

/// Smallest body worth compressing: below it the answer fits in one packet either way.
pub(super) const MIN_BYTES: usize = 1024;

/// Largest body compressed in one pass; larger ones stream through the layer.
const MAX_BYTES: usize = 8 << 20;

/// The deflate level of dynamic responses (a 362 KB grid block becomes 37 KB for 0.8 ms of CPU;
/// level 1 gives 54 KB for 0.6 ms and level 6 35 KB for 1.4 ms).
pub(super) const LEVEL: u32 = 2;

thread_local! {
    /// The compressor of this thread, made on its first response and reset after each.
    static DEFLATE: RefCell<Option<Compress>> = const { RefCell::new(None) };
}

/// Whether the request accepts gzip (`Accept-Encoding: gzip`, not with `q=0`).
pub(super) fn accepts_gzip(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|e| {
            let mut p = e.split(';');
            p.next()
                .is_some_and(|n| n.trim().eq_ignore_ascii_case("gzip"))
                && !p.any(|q| {
                    q.trim()
                        .strip_prefix("q=")
                        .is_some_and(|v| {
                            v.trim()
                                .parse::<f32>()
                                .is_ok_and(|v| v == 0.0)
                        })
                })
        })
}

/// `bytes` as a gzip member: header, raw deflate with this thread's compressor, CRC and length.
fn gzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len() / 4 + 64);
    out.extend_from_slice(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff]);
    DEFLATE.with_borrow_mut(|slot| {
        let z = slot.get_or_insert_with(|| Compress::new(Compression::new(LEVEL), false));
        z.reset();
        let start = z.total_in();
        loop {
            let read = usize::try_from(z.total_in() - start).ok()?;
            if out.capacity() - out.len() < 64 {
                out.reserve(bytes.len() / 8 + 64);
            }
            match z
                .compress_vec(&bytes[read..], &mut out, FlushCompress::Finish)
                .ok()?
            {
                Status::StreamEnd => break Some(()),
                Status::Ok => {}
                Status::BufError => out.reserve(bytes.len() / 8 + 64),
            }
        }
    })?;
    let mut crc = Crc::new();
    crc.update(bytes);
    out.extend_from_slice(&crc.sum().to_le_bytes());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    Some(out)
}

/// Compresses a whole response when the request accepts gzip, the body's size is known and
/// between [`MIN_BYTES`] and 8 MB, and `compressible` allows its status and headers. Anything else
/// passes unchanged to the streaming layer, which skips what this already encoded.
pub(super) async fn whole(
    req: axum::extract::Request,
    next: axum::middleware::Next,
    compressible: fn(StatusCode, &HeaderMap) -> bool,
) -> Response {
    let gzip_ok = accepts_gzip(req.headers());
    let res = next.run(req).await;
    let size = res
        .body()
        .size_hint()
        .exact()
        .and_then(|n| usize::try_from(n).ok());
    let fits = size.is_some_and(|n| (MIN_BYTES..=MAX_BYTES).contains(&n));
    if !gzip_ok
        || !fits
        || res
            .headers()
            .contains_key(header::CONTENT_ENCODING)
        || res
            .headers()
            .contains_key(header::CONTENT_RANGE)
        || !compressible(res.status(), res.headers())
    {
        return res;
    }
    let (mut parts, body) = res.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_BYTES).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(packed) = gzip(&bytes) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let h = &mut parts.headers;
    h.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    h.append(header::VARY, HeaderValue::from_static("accept-encoding"));
    h.insert(header::CONTENT_LENGTH, HeaderValue::from(packed.len()));
    Response::from_parts(parts, Body::from(Bytes::from(packed)))
}
