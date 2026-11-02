//! Memory of an upload stays flat as the file grows: the body is streamed to the store in
//! bounded chunks. Its own test binary, so no other test allocates while it measures.

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{BoxFuture, FileChunks, FileMeta, FileStore, I18n, Identity, StoredFile};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const BIG: FileRules = FileRules::new("big").max_bytes(1 << 30).accept(&[FileType::Png]);
const CHUNK: usize = 64 * 1024;

/// Counts the bytes it is given and keeps none of them.
#[derive(Clone, Default)]
struct Counter(Arc<AtomicU64>);

impl FileStore for Counter {
    fn put(&self, _meta: FileMeta, mut chunks: FileChunks) -> BoxFuture<Result<()>> {
        let n = self.0.clone();
        Box::pin(async move {
            while let Some(c) = chunks.next().await {
                n.fetch_add(c.map_err(|e| Error::Internal(e.to_string()))?.len() as u64, Ordering::Relaxed);
            }
            Ok(())
        })
    }
    fn get(&self, _id: String) -> BoxFuture<Result<StoredFile>> {
        Box::pin(async { Err(Error::NotFound) })
    }
    fn meta(&self, _id: String) -> BoxFuture<Result<FileMeta>> {
        Box::pin(async { Err(Error::NotFound) })
    }
    fn delete(&self, _id: String) -> BoxFuture<Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// A PNG header followed by zeros, made one chunk at a time.
fn body(total: usize) -> Body {
    let stream = futures_util::stream::unfold(0usize, move |sent| async move {
        if sent >= total {
            return None;
        }
        let n = CHUNK.min(total - sent);
        let mut chunk = vec![0u8; n];
        if sent == 0 {
            chunk[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        }
        Some((Ok::<_, std::io::Error>(Bytes::from(chunk)), sent + n))
    });
    Body::from_stream(stream)
}

async fn upload(router: &axum::Router, total: usize) -> (StatusCode, usize) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let req = Request::post("/_zeyn/files/big")
        .header("x-zeyn-csrf", TOKEN)
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-file-name", "huge.png")
        .body(body(total))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let _ = res.into_body().collect().await;
    (status, PEAK.load(Ordering::Relaxed).saturating_sub(base))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_large_upload_keeps_memory_flat() {
    let counter = Counter::default();
    let dir = std::env::temp_dir().join(format!("zeyn-files-memory-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let router = |store: Box<dyn Fn(Feature) -> Feature>| {
        App::new("Test")
            .i18n(I18n::new("en"))
            .auth(|_: &HeaderMap| Some(Identity::user("ada").with_permissions(["files.write"])))
            .feature(store(Feature::new("files")))
            .router()
            .unwrap()
    };
    let counted = {
        let c = counter.clone();
        router(Box::new(move |f| f.files(BIG, "files.write", c.clone())))
    };
    assert_eq!(upload(&counted, 1 << 20).await.0, StatusCode::CREATED, "warm up");
    let (status, small) = upload(&counted, 8 << 20).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, large) = upload(&counted, 256 << 20).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(counter.0.load(Ordering::Relaxed), (1 << 20) + (8 << 20) + (256 << 20));
    println!("peak heap growth: 8 MiB upload {} KiB, 256 MiB upload {} KiB", small / 1024, large / 1024);
    assert!(large < 4 << 20, "a 256 MiB upload must not hold more than 4 MiB, held {large} bytes");

    let disk = {
        let d = dir.clone();
        router(Box::new(move |f| f.files(BIG, "files.write", zeyn::DiskStore::new(d.clone()))))
    };
    let (status, on_disk) = upload(&disk, 64 << 20).await;
    assert_eq!(status, StatusCode::CREATED);
    println!("peak heap growth: 64 MiB upload to disk {} KiB", on_disk / 1024);
    assert!(on_disk < 4 << 20, "a 64 MiB upload to disk must not hold more than 4 MiB, held {on_disk} bytes");
    let _ = std::fs::remove_dir_all(&dir);
}
