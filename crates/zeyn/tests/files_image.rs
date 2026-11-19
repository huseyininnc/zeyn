//! Image cleaning on upload (feature `image`): PNG, JPEG and WebP are decoded and encoded again,
//! so metadata and hidden trailing bytes never reach the store, and the EXIF orientation turns
//! the pixels. The fixtures in `tests/images` carry a camera name in EXIF or a text chunk and
//! `HIDDEN-TAIL` after the image data; `photo.jpg` is 4 by 2 pixels with orientation 6.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{FileStore, I18n, Identity, MemoryStore};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

const PHOTOS: FileRules = FileRules::new("photos")
    .accept(&[FileType::Png, FileType::Jpeg, FileType::Webp, FileType::Pdf])
    .clean_images();

const RAW: FileRules = FileRules::new("raw").accept(&[FileType::Jpeg]);

const JPEG: &[u8] = include_bytes!("images/photo.jpg");
const PNG: &[u8] = include_bytes!("images/pic.png");
const WEBP: &[u8] = include_bytes!("images/pic.webp");

/// The EXIF entry that points at the GPS block: tag 0x8825, type LONG, in big endian.
const GPS: &[u8] = &[0x88, 0x25, 0x00, 0x04];

fn app(store: MemoryStore) -> Router {
    App::new("Images")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("ada").with_permissions(["photos"])))
        .feature(
            Feature::new("photos")
                .files(PHOTOS, "photos", store)
                .files(RAW, "photos", MemoryStore::default()),
        )
        .router()
        .unwrap()
}

async fn upload(router: &Router, store: &str, name: &str, body: &[u8]) -> (StatusCode, Vec<u8>) {
    let res = router
        .clone()
        .oneshot(
            Request::post(format!("/_zeyn/files/{store}"))
                .header("x-zeyn-csrf", TOKEN)
                .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
                .header("x-zeyn-file-name", name)
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, bytes.to_vec())
}

async fn stored(store: &MemoryStore, id: &str) -> Vec<u8> {
    let mut file = store.get(id.to_string()).await.unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut file.reader, &mut bytes).unwrap();
    bytes
}

fn has(bytes: &[u8], needle: &[u8]) -> bool {
    bytes
        .windows(needle.len())
        .any(|w| w == needle)
}

fn id(body: &[u8]) -> String {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap();
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn the_fixtures_carry_metadata_and_a_hidden_tail() {
    for f in [JPEG, PNG, WEBP] {
        assert!(has(f, b"SecretCam"));
    }
    assert!(has(JPEG, b"HIDDEN-TAIL") && has(PNG, b"HIDDEN-TAIL"));
    assert!(has(JPEG, GPS) && has(WEBP, GPS), "a GPS block in EXIF");
}

#[tokio::test]
async fn uploaded_images_lose_metadata_and_hidden_bytes() {
    let store = MemoryStore::default();
    let router = app(store.clone());
    for (name, body, magic) in [
        ("photo.jpg", JPEG, &b"\xff\xd8\xff"[..]),
        ("pic.png", PNG, &b"\x89PNG"[..]),
        ("pic.webp", WEBP, &b"RIFF"[..]),
    ] {
        let (status, answer) = upload(&router, "photos", name, body).await;
        assert_eq!(status, StatusCode::CREATED, "{name}");
        let bytes = stored(&store, &id(&answer)).await;
        assert!(bytes.starts_with(magic), "{name} keeps its format");
        assert!(!has(&bytes, b"SecretCam"), "{name} lost its metadata");
        assert!(!has(&bytes, b"HIDDEN-TAIL"), "{name} lost its tail");
        assert!(!has(&bytes, b"Exif"), "{name} has no EXIF block");
        assert!(!has(&bytes, GPS), "{name} lost its GPS block");
        let v: serde_json::Value = serde_json::from_slice(&answer).unwrap();
        assert_eq!(v["size"], bytes.len() as u64, "{name} answers the new size");
    }
    assert_eq!(store.ids().len(), 3, "the originals are gone");
}

#[tokio::test]
async fn the_exif_orientation_turns_the_pixels() {
    let store = MemoryStore::default();
    let router = app(store.clone());
    let (_, answer) = upload(&router, "photos", "photo.jpg", JPEG).await;
    let bytes = stored(&store, &id(&answer)).await;
    let sof = bytes
        .windows(2)
        .position(|w| w == [0xff, 0xc0])
        .expect("a baseline frame header");
    let height = u16::from_be_bytes([bytes[sof + 5], bytes[sof + 6]]);
    let width = u16::from_be_bytes([bytes[sof + 7], bytes[sof + 8]]);
    assert_eq!((width, height), (2, 4), "orientation 6 is a quarter turn");
}

#[tokio::test]
async fn an_image_that_does_not_decode_is_refused_and_nothing_is_kept() {
    let store = MemoryStore::default();
    let router = app(store.clone());
    let mut broken = PNG[..40].to_vec();
    broken.extend_from_slice(&[0u8; 64]);
    let (status, _) = upload(&router, "photos", "broken.png", &broken).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(store.ids().is_empty());
}

#[tokio::test]
async fn other_files_and_stores_without_cleaning_stay_as_sent() {
    let store = MemoryStore::default();
    let router = app(store.clone());
    let pdf = b"%PDF-1.7\n% SecretCam\n%%EOF\n";
    let (status, answer) = upload(&router, "photos", "a.pdf", pdf).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(stored(&store, &id(&answer)).await, pdf.to_vec());
    let (status, _) = upload(&router, "raw", "photo.jpg", JPEG).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a store without clean_images takes it as is"
    );
}

/// CRC-32 of a PNG chunk.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn chunk(png: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

/// A valid PNG of `width` by `height` black pixels, 8 bits grey (`colour` 0) or RGBA
/// (`colour` 6). One row is compressed once with a full flush and its deflate blocks repeated
/// for every row, so a picture of hundreds of megabytes fits in a few megabytes, the
/// way a decompression bomb does.
fn png(width: u32, height: u32, colour: u8) -> Vec<u8> {
    let channels = if colour == 6 { 4 } else { 1 };
    let row = vec![0u8; 1 + width as usize * channels];
    let mut deflate = flate2::Compress::new(flate2::Compression::fast(), false);
    let mut block = Vec::with_capacity(row.len());
    deflate
        .compress_vec(&row, &mut block, flate2::FlushCompress::Full)
        .unwrap();
    let mut zlib = vec![0x78, 0x01];
    for _ in 0..height {
        zlib.extend_from_slice(&block);
    }
    zlib.extend_from_slice(&[0x03, 0x00]);
    let total = row.len() as u64 * u64::from(height);
    let adler = ((total % 65_521) << 16) as u32 | 1;
    zlib.extend_from_slice(&adler.to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, colour, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[test]
fn the_test_images_decode_below_the_limits() {
    assert!(zeyn::clean_image(FileType::Png, &png(2_000, 3, 0)).is_some());
    assert!(zeyn::clean_image(FileType::Png, &png(300, 200, 6)).is_some());
}

#[test]
fn an_image_wider_than_the_limit_is_refused_from_its_header() {
    let wide = png(zeyn::MAX_IMAGE_SIDE + 1, 1, 0);
    assert!(zeyn::clean_image(FileType::Png, &wide).is_none());
}

#[test]
fn a_decompression_bomb_is_refused_before_it_is_decoded() {
    let bomb = png(9_000, 9_000, 6);
    assert!(bomb.len() < 4 * 1024 * 1024, "{} bytes", bomb.len());
    let started = std::time::Instant::now();
    assert!(zeyn::clean_image(FileType::Png, &bomb).is_none());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "the 324 MB of pixels are never inflated"
    );
}

#[tokio::test]
async fn an_oversized_upload_is_refused_and_nothing_is_kept() {
    let store = MemoryStore::default();
    let router = app(store.clone());
    let wide = png(zeyn::MAX_IMAGE_SIDE + 1, 1, 0);
    let (status, _) = upload(&router, "photos", "wide.png", &wide).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(store.ids().is_empty());
}
