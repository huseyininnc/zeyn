//! Media serving and players: range requests, access, ids, content types, markup and resume.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{BoxFuture, Component, I18n, Identity, Media, MediaSource, Render};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Bytes `0, 1, ... 255, 0, 1, ...` so any slice can be checked by position.
fn bytes(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i % 256) as u8)
        .collect()
}

/// Saved positions by user and media id.
type Positions = Arc<Mutex<HashMap<(String, String), f64>>>;

#[derive(Clone, Default)]
struct Files {
    opened: Arc<Mutex<Vec<String>>>,
    positions: Option<Positions>,
}

impl MediaSource for Files {
    fn open(&self, _ctx: Ctx, id: String) -> BoxFuture<Result<Media>> {
        self.opened
            .lock()
            .unwrap()
            .push(id.clone());
        Box::pin(async move {
            match id.as_str() {
                "clip.mp4" => Ok(Media::bytes(bytes(1000), "video/mp4").etag("v1")),
                "big.webm" => Ok(Media::bytes(bytes(200_000), "video/webm")),
                "notes.vtt" => Ok(Media::bytes(notes(), "text/vtt")),
                "page.html" => Ok(Media::bytes(
                    b"<script>alert(1)</script>".to_vec(),
                    "text/html",
                )),
                _ => Err(Error::NotFound),
            }
        })
    }

    fn position(&self, ctx: Ctx, id: String) -> BoxFuture<Result<Option<f64>>> {
        let at = self.positions.as_ref().map(|p| {
            p.lock()
                .unwrap()
                .get(&(ctx.identity().user.clone().unwrap(), id))
                .copied()
        });
        Box::pin(async move { at.ok_or(Error::NotFound) })
    }

    fn save_position(&self, ctx: Ctx, id: String, seconds: f64) -> BoxFuture<Result<()>> {
        let saved = self.positions.as_ref().map(|p| {
            p.lock()
                .unwrap()
                .insert((ctx.identity().user.clone().unwrap(), id), seconds);
        });
        Box::pin(async move { saved.ok_or(Error::NotFound) })
    }
}

#[screen(route = "/watch", permission = "media.play")]
pub fn watch() -> Screen {
    Screen::new("Watch").body(
        Row::new().col(
            Video::new("films", "Clip")
                .file("clip.mp4")
                .captions("clip.en.vtt", "en", "English"),
        ),
    )
}

fn no_closed(ctx: &Ctx) -> GuardResult {
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

fn app(films: Files) -> Router {
    let music = Files::default();
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
        .feature(
            Feature::new("films")
                .guard(no_closed)
                .media("films", "media.play", films)
                .media("music", "music.play", music)
                .screen(watch),
        )
        .router()
        .unwrap()
}

fn store() -> Files {
    Files {
        positions: Some(Arc::default()),
        ..Files::default()
    }
}

fn get(path: &str, perms: &str) -> axum::http::request::Builder {
    Request::get(path)
        .header("x-user", "ada")
        .header("x-perms", perms)
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

fn h(headers: &HeaderMap, name: header::HeaderName) -> &str {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

#[tokio::test]
async fn full_request_has_length_type_ranges_and_nosniff() {
    let router = app(Files::default());
    let (status, headers, body) = send(
        &router,
        get("/_zeyn/media/films/clip.mp4", "media.play")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, bytes(1000));
    assert_eq!(h(&headers, header::CONTENT_TYPE), "video/mp4");
    assert_eq!(h(&headers, header::CONTENT_LENGTH), "1000");
    assert_eq!(h(&headers, header::ACCEPT_RANGES), "bytes");
    assert_eq!(h(&headers, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(h(&headers, header::ETAG), "\"v1\"");
}

#[tokio::test]
async fn ranges_answer_partial_content() {
    let router = app(Files::default());
    let all = bytes(1000);
    for (range, first, last) in [
        ("bytes=0-99", 0, 99),
        ("bytes=-500", 500, 999),
        ("bytes=900-", 900, 999),
        ("bytes=990-5000", 990, 999),
    ] {
        let req = get("/_zeyn/media/films/clip.mp4", "media.play")
            .header(header::RANGE, range)
            .body(Body::empty())
            .unwrap();
        let (status, headers, body) = send(&router, req).await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT, "{range}");
        assert_eq!(
            h(&headers, header::CONTENT_RANGE),
            format!("bytes {first}-{last}/1000"),
            "{range}"
        );
        assert_eq!(
            h(&headers, header::CONTENT_LENGTH),
            (last - first + 1).to_string(),
            "{range}"
        );
        assert_eq!(body, all[first..=last].to_vec(), "{range}");
    }
}

#[tokio::test]
async fn bad_ranges_answer_416() {
    let router = app(Files::default());
    for range in ["bytes=1000-", "bytes=abc", "bytes=5-1", "bytes=-0"] {
        let req = get("/_zeyn/media/films/clip.mp4", "media.play")
            .header(header::RANGE, range)
            .body(Body::empty())
            .unwrap();
        let (status, headers, body) = send(&router, req).await;
        assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE, "{range}");
        assert_eq!(h(&headers, header::CONTENT_RANGE), "bytes */1000");
        assert!(body.is_empty());
    }
}

#[tokio::test]
async fn if_range_and_if_none_match_follow_the_entity_tag() {
    let router = app(Files::default());
    let stale = get("/_zeyn/media/films/clip.mp4", "media.play")
        .header(header::RANGE, "bytes=0-9")
        .header(header::IF_RANGE, "\"old\"");
    assert_eq!(
        send(&router, stale.body(Body::empty()).unwrap())
            .await
            .0,
        StatusCode::OK
    );
    let fresh = get("/_zeyn/media/films/clip.mp4", "media.play")
        .header(header::RANGE, "bytes=0-9")
        .header(header::IF_RANGE, "\"v1\"");
    assert_eq!(
        send(&router, fresh.body(Body::empty()).unwrap())
            .await
            .0,
        StatusCode::PARTIAL_CONTENT
    );
    let cached =
        get("/_zeyn/media/films/clip.mp4", "media.play").header(header::IF_NONE_MATCH, "\"v1\"");
    let (status, _, body) = send(&router, cached.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
}

#[tokio::test]
async fn large_files_stream_in_chunks_with_exact_bytes() {
    let router = app(Files::default());
    let (status, _, body) = send(
        &router,
        get("/_zeyn/media/films/big.webm", "media.play")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, bytes(200_000));
    let req = get("/_zeyn/media/films/big.webm", "media.play")
        .header(header::RANGE, "bytes=65000-140000")
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send(&router, req).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, bytes(200_000)[65000..=140000].to_vec());
}

#[tokio::test]
async fn permission_guards_and_unknown_ids_are_refused() {
    let router = app(Files::default());
    let req = |path: &str, perms: &str| {
        get(path, perms)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&router, req("/_zeyn/media/films/clip.mp4", ""))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, req("/_zeyn/media/music/clip.mp4", "media.play"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &router,
            req("/_zeyn/media/films/clip.mp4", "media.play,closed")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, req("/_zeyn/media/films/missing.mp4", "media.play"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&router, req("/_zeyn/media/nowhere/clip.mp4", "media.play"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn traversal_never_reaches_the_source() {
    let files = Files::default();
    let router = app(files.clone());
    for path in [
        "/_zeyn/media/films/..%2F..%2Fetc%2Fpasswd",
        "/_zeyn/media/films/..",
        "/_zeyn/media/films/.env",
        "/_zeyn/media/films/a%5Cb",
        "/_zeyn/media/films/..%5Cwin.ini",
    ] {
        let (status, _, _) = send(
            &router,
            get(path, "media.play")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    assert!(files.opened.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unsafe_content_types_download_instead_of_rendering() {
    let router = app(Files::default());
    let (status, headers, _) = send(
        &router,
        get("/_zeyn/media/films/page.html", "media.play")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        h(&headers, header::CONTENT_TYPE),
        "application/octet-stream"
    );
    assert_eq!(h(&headers, header::CONTENT_DISPOSITION), "attachment");
}

#[test]
fn videos_load_nothing_before_play_and_previews_are_muted_with_a_placeholder_stage() {
    let r = Render::english("s");
    let mut html = String::new();
    Video::new("films", "Shirt in motion")
        .sources([
            ("shirt-480.mp4", "(max-width: 600px)"),
            ("shirt-bad.mp4", r#"all" onerror="x"#),
            ("../secret.mp4", "all"),
        ])
        .file("shirt-1080.mp4")
        .poster("shirt.jpg")
        .poster_low("shirt-20.jpg")
        .placeholder(Placeholder::color("#d9dde3"))
        .preview_when_visible()
        .on_error(Text::from("Video <unavailable>"))
        .render(&mut html, &r);
    assert!(html.contains(r#"data-zeyn-preview>"#), "{html}");
    assert!(html.contains(
        r#"<div class="z-player-stage z-img-host" style="--z-ph:#d9dde3"><video class="z-player-media" preload="none" playsinline muted loop aria-label="Shirt in motion" data-zeyn-poster="/_zeyn/media/films/shirt.jpg" data-zeyn-poster-low="/_zeyn/media/films/shirt-20.jpg">"#
    ), "{html}");
    assert!(html.contains(
        r#"<source data-zeyn-src="/_zeyn/media/films/shirt-480.mp4" type="video/mp4" media="(max-width: 600px)"><source data-zeyn-src="/_zeyn/media/films/shirt-1080.mp4" type="video/mp4">"#
    ));
    assert!(!html.contains("shirt-bad") && !html.contains("secret") && !html.contains(" poster="));
    assert!(html.contains(
        r#"<div class="z-player-error" data-zeyn-player-error hidden>Video &lt;unavailable&gt;</div>"#
    ));
    let mut audio = String::new();
    Audio::new("music", "Song")
        .file("song.mp3")
        .render(&mut audio, &r);
    assert!(audio.contains(r#"preload="none""#) && !audio.contains("data-zeyn-preview"));
}

#[test]
fn players_render_accessible_controls_and_escape_text() {
    let r = Render::english("s");
    let mut html = String::new();
    Video::new("films", "<Clip>")
        .file("clip.webm")
        .file("../secret")
        .hls("clip.m3u8")
        .poster("clip.jpg")
        .captions("clip.en.vtt", "en", "English")
        .captions("clip.tr.vtt", "tr", "Türkçe")
        .render(&mut html, &r);
    assert!(html.contains(r#"data-zeyn-src="/_zeyn/media/films/clip.webm" type="video/webm""#));
    assert!(!html.contains("secret"));
    assert!(html.contains(r#"data-zeyn-hls="/_zeyn/media/films/clip.m3u8""#));
    assert!(html.contains(r#"data-zeyn-poster="/_zeyn/media/films/clip.jpg""#));
    assert!(html.contains(
        r#"<track kind="subtitles" data-zeyn-src="/_zeyn/media/films/clip.en.vtt" srclang="en" label="English">"#
    ));
    assert!(html.contains("&lt;Clip&gt;") && !html.contains("<Clip>"));
    assert!(html.contains(r#"role="slider" tabindex="0" data-zeyn-player-seek aria-label="Seek""#));
    assert!(html.contains(r#"data-zeyn-player-volume aria-label="Volume""#));
    assert!(html.contains(r#"aria-haspopup="menu""#) && html.contains(r#"role="menuitemradio""#));
    assert!(
        html.contains("data-zeyn-player-cc")
            && html.contains("data-zeyn-player-fullscreen")
            && html.contains("data-zeyn-player-pip")
    );
    assert!(html.contains(r#"data-zeyn-media-id="clip.webm""#));

    let mut audio = String::new();
    Audio::new("music", "Song")
        .subtitle("Band")
        .file("song.mp3")
        .render(&mut audio, &r);
    assert!(audio.contains("<audio") && audio.contains("Band"));
    assert!(
        !audio.contains("data-zeyn-player-fullscreen") && !audio.contains("data-zeyn-player-cc")
    );
}

#[tokio::test]
async fn page_loads_the_player_assets() {
    let router = app(Files::default());
    let (status, _, body) = send(
        &router,
        get("/watch", "media.play")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("/_zeyn/media.css"));
    assert!(
        html.contains(r#"<script type="module" src="/_zeyn/media.js?v="#),
        "a page that holds a player links the player script"
    );
    assert!(html.contains(r#"data-zeyn-player="video""#));
    let (status, headers, _) = send(
        &router,
        Request::get("/_zeyn/media.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(h(&headers, header::CONTENT_TYPE).starts_with("text/javascript"));
}

fn save(perms: &str, csrf: bool, at: serde_json::Value) -> Request<Body> {
    let mut b = Request::post("/_zeyn/media/position")
        .header("x-user", "ada")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json");
    if csrf {
        b = b
            .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
            .header("x-zeyn-csrf", TOKEN);
    }
    b.body(Body::from(
        serde_json::json!({ "source": "films", "id": "clip.mp4", "at": at }).to_string(),
    ))
    .unwrap()
}

#[tokio::test]
async fn positions_need_csrf_permission_and_a_store() {
    let router = app(store());
    assert_eq!(
        send(&router, save("media.play", false, 42.5.into()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, save("", true, 42.5.into()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, save("media.play", true, (-1).into()))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&router, save("media.play", true, 1e12.into()))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        send(&router, save("media.play", true, 42.5.into()))
            .await
            .0,
        StatusCode::NO_CONTENT
    );

    let read = |perms: &str| {
        get("/_zeyn/media/position?source=films&id=clip.mp4", perms)
            .body(Body::empty())
            .unwrap()
    };
    let (status, _, body) = send(&router, read("media.play")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({ "at": 42.5, "kept": true })
    );
    assert_eq!(send(&router, read("")).await.0, StatusCode::FORBIDDEN);
    let traversal = get("/_zeyn/media/position?source=films&id=..%2Fx", "media.play")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&router, traversal).await.0, StatusCode::NOT_FOUND);

    let unknown = get(
        "/_zeyn/media/position?source=nowhere&id=clip.mp4",
        "media.play",
    )
    .body(Body::empty())
    .unwrap();
    assert_eq!(send(&router, unknown).await.0, StatusCode::NOT_FOUND);

    let without = app(Files::default());
    let (status, _, body) = send(&without, read("media.play")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({ "at": null, "kept": false })
    );
    assert_eq!(
        send(&without, save("media.play", true, 3.into()))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn media_source_names_are_checked() {
    let twice = App::new("T")
        .feature(Feature::new("a").media("films", "p", Files::default()))
        .feature(Feature::new("b").media("films", "p", Files::default()))
        .check()
        .unwrap_err();
    assert!(
        twice
            .iter()
            .any(|p| p.contains("media source `films`")),
        "{twice:?}"
    );
    let bad = App::new("T")
        .feature(Feature::new("a").media("../x", "p", Files::default()))
        .check()
        .unwrap_err();
    assert!(
        bad.iter()
            .any(|p| p.contains("media source `../x`")),
        "{bad:?}"
    );
}

fn public_app(images: Files) -> Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .feature(
            Feature::new("catalog")
                .public_media("images", 86_400, images)
                .media("films", "media.play", Files::default()),
        )
        .router()
        .unwrap()
}

#[tokio::test]
async fn public_media_is_served_to_anyone_with_a_shared_cache_and_no_cookie() {
    let router = public_app(Files::default());
    let req = Request::get("/_zeyn/media/images/clip.mp4")
        .body(Body::empty())
        .unwrap();
    let (status, headers, body) = send(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, bytes(1000));
    assert_eq!(h(&headers, header::CACHE_CONTROL), "public, max-age=86400");
    assert_eq!(h(&headers, header::ETAG), "\"v1\"");
    assert!(
        headers
            .get(header::SET_COOKIE)
            .is_none()
    );
    let cached =
        Request::get("/_zeyn/media/images/clip.mp4").header(header::IF_NONE_MATCH, "\"v1\"");
    assert_eq!(
        send(&router, cached.body(Body::empty()).unwrap())
            .await
            .0,
        StatusCode::NOT_MODIFIED
    );
    let unknown = Request::get("/_zeyn/media/images/../x")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&router, unknown).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn private_media_next_to_public_media_still_needs_its_permission() {
    let router = public_app(Files::default());
    let req = Request::get("/_zeyn/media/films/clip.mp4")
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = send(&router, req).await;
    assert_ne!(status, StatusCode::OK);
    assert_ne!(h(&headers, header::CACHE_CONTROL), "public, max-age=86400");
}

#[tokio::test]
async fn public_media_reads_skip_the_rate_limit_but_private_media_and_ip_rules_still_apply() {
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .rate_limit(zeyn::RateLimit::per_ip(
            2,
            std::time::Duration::from_secs(60),
        ))
        .deny_ips(["198.51.100.0/24"])
        .feature(
            Feature::new("catalog")
                .public_media("images", 86_400, Files::default())
                .media("films", "media.play", Files::default()),
        )
        .router()
        .unwrap();
    let from = |path: &str, ip: &str| {
        let mut req = Request::get(path)
            .body(Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo(std::net::SocketAddr::new(
                ip.parse().unwrap(),
                5000,
            )));
        req
    };
    for _ in 0..30 {
        assert_eq!(
            send(&router, from("/_zeyn/media/images/clip.mp4", "203.0.113.5"))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        send(&router, from("/_zeyn/media/films/clip.mp4", "203.0.113.5"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, from("/_zeyn/media/films/clip.mp4", "203.0.113.5"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, from("/_zeyn/media/films/clip.mp4", "203.0.113.5"))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS,
        "private media still counts"
    );
    assert_eq!(
        send(
            &router,
            from("/_zeyn/media/images/a/clip.mp4", "203.0.113.5")
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS,
        "only a file of a public source skips the limit"
    );
    assert_eq!(
        send(&router, from("/_zeyn/media/images/clip.mp4", "203.0.113.5"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            from("/_zeyn/media/images/clip.mp4", "198.51.100.7")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[test]
fn public_media_cache_is_capped_at_a_year() {
    let f = Feature::new("c").public_media("p", u32::MAX, Files::default());
    assert_eq!(
        f.media_sources()[0].public_cache,
        Some(zeyn::MAX_PUBLIC_MEDIA_CACHE)
    );
}

#[tokio::test]
async fn ranges_and_media_are_never_compressed() {
    let router = app(Files::default());
    let gz = |path: &str, range: Option<&str>| {
        let mut req = get(path, "media.play").header(header::ACCEPT_ENCODING, "gzip");
        if let Some(r) = range {
            req = req.header(header::RANGE, r);
        }
        send(&router, req.body(Body::empty()).unwrap())
    };
    let (status, headers, body) = gz("/_zeyn/media/films/notes.vtt", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        h(&headers, header::CONTENT_ENCODING),
        "gzip",
        "captions of 5 KB are compressed without a range"
    );
    assert!(body.len() < 5000);

    let (status, headers, body) = gz("/_zeyn/media/films/notes.vtt", Some("bytes=100-2099")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        h(&headers, header::CONTENT_ENCODING),
        "",
        "a ranged request is never compressed"
    );
    assert_eq!(h(&headers, header::CONTENT_RANGE), "bytes 100-2099/5000");
    assert_eq!(body, notes()[100..2100]);

    let (status, headers, body) = gz("/_zeyn/media/films/big.webm", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        h(&headers, header::CONTENT_ENCODING),
        "",
        "video is never compressed"
    );
    assert_eq!(h(&headers, header::CONTENT_LENGTH), "200000");
    assert_eq!(body, bytes(200_000));
}

/// Five thousand bytes of caption text.
fn notes() -> Vec<u8> {
    "line of notes\n"
        .repeat(400)
        .into_bytes()[..5000]
        .to_vec()
}
