//! The social pack: story rings, the lightbox and the feed.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::MAX_STORY_RINGS;
use zeyn::{Component, I18n, Identity, Render};

fn html(c: &impl Component, locale: &str) -> String {
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

#[params]
pub struct PersonParams {
    pub id: i64,
}

#[screen(route = "/people/{id}/stories", permission = "social.read")]
pub fn person_stories(p: PersonParams) -> Screen {
    Screen::new("Stories").body(Paragraph::new(p.id.to_string()))
}

#[screen(route = "/plain", permission = "social.read")]
pub fn only_text() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/stories", permission = "social.read")]
pub fn stories() -> Screen {
    Screen::new("Stories").body(
        Stack::new().add(
            StoryRow::new().add(
                StoryRing::new("Ada Lovelace", StoryState::Unseen)
                    .link(person_stories, PersonParams { id: 1 }),
            ),
        ),
    )
}

#[screen(route = "/photos", permission = "social.read")]
pub fn photos() -> Screen {
    Screen::new("Photos").body(
        Row::new()
            .col(
                Image::new("/p/1-small.webp")
                    .alt("One")
                    .size(320, 240)
                    .full("/p/1.webp"),
            )
            .col(
                Image::new("/p/2-small.webp")
                    .alt("Two")
                    .size(320, 240)
                    .lightbox(),
            ),
    )
}

#[screen(route = "/photo", permission = "social.read")]
pub fn photo() -> Screen {
    Screen::new("Photo").body(
        Image::new("/p/1-small.webp")
            .alt("One")
            .size(320, 240),
    )
}

fn feature() -> Feature {
    Feature::new("social")
        .screen(person_stories)
        .screen(only_text)
        .screen(stories)
        .screen(photos)
        .screen(photo)
}

fn app() -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("social.read")
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(feature())
        .router()
        .unwrap()
}

async fn get(router: &axum::Router, path: &str, perms: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-perms", perms)
                .body(Body::empty())
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
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// Whether a page links the framework file `name`, such as `feed.js` or `stories.css`.
fn links(page: &str, name: &str) -> bool {
    page.contains(&format!("/_zeyn/{name}?"))
}

#[test]
fn a_story_ring_names_the_person_and_the_state() {
    let unseen = html(
        &StoryRing::new("Ada <Lovelace>", StoryState::Unseen)
            .image("/img/ada.webp")
            .link(person_stories, PersonParams { id: 7 }),
        "en",
    );
    assert!(
        unseen.starts_with(r#"<a class="z-story z-story-unseen" href="/_zeyn/route/person_stories?params=%7B%22id%22%3A7%7D" data-zeyn-base-href aria-label="Ada &lt;Lovelace&gt;, unseen stories">"#),
        "{unseen}"
    );
    assert!(unseen.contains(r#"<img class="z-story-face" src="/img/ada.webp" alt="""#));
    assert!(
        unseen.contains(
            r#"<span class="z-story-name" aria-hidden="true">Ada &lt;Lovelace&gt;</span>"#
        ),
        "the name is escaped: {unseen}"
    );
    let seen = html(&StoryRing::new("İlknur Şahin", StoryState::Seen), "tr");
    assert!(
        seen.starts_with(r#"<span class="z-story z-story-seen" role="img" aria-label="İlknur Şahin, tüm hikâyeler izlendi">"#),
        "{seen}"
    );
    assert!(seen.contains(r#"<span class="z-story-face z-story-initials">İŞ</span>"#));
    let blocked = html(
        &StoryRing::new("Ada", StoryState::Seen).image("javascript:alert(1)"),
        "en",
    );
    assert!(
        !blocked.contains("javascript") && blocked.contains("z-story-initials"),
        "{blocked}"
    );
}

#[test]
fn a_story_row_is_a_named_list_with_a_cap() {
    let row = (0..MAX_STORY_RINGS + 5).fold(StoryRow::new(), |row, i| {
        row.add(StoryRing::new(format!("P{i}"), StoryState::Unseen))
    });
    let out = html(&row, "en");
    assert!(
        out.starts_with(r#"<ul class="z-stories" aria-label="Stories" tabindex="0""#),
        "rings without links leave the row itself to the keyboard: {out}"
    );
    assert_eq!(out.matches("<li>").count(), MAX_STORY_RINGS);
    let linked = html(
        &StoryRow::new().label("Friends").add(
            StoryRing::new("Ada", StoryState::Unseen).link(person_stories, PersonParams { id: 1 }),
        ),
        "en",
    );
    assert!(
        linked.starts_with(r#"<ul class="z-stories" aria-label="Friends" data-testid"#),
        "{linked}"
    );
}

#[tokio::test]
async fn story_styles_load_only_on_pages_with_rings() {
    let router = app();
    let (status, page) = get(&router, "/stories", "social.read").await;
    assert_eq!(status, StatusCode::OK);
    assert!(links(&page, "stories.css"), "{page}");
    assert!(!links(&page, "stories.js"), "rings need no script");
    let (_, text) = get(&router, "/plain", "social.read").await;
    assert!(!links(&text, "stories.css"));
    let (status, _) = get(&router, "/stories", "other").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the screen's permission");
}

#[test]
fn a_lightbox_image_is_a_link_to_its_larger_file() {
    let out = html(
        &Image::new("/p/1-small.webp")
            .alt("Harbor <dawn>")
            .size(320, 240)
            .full("/p/1.webp")
            .caption("From the \"old\" lighthouse"),
        "en",
    );
    assert!(
        out.starts_with(r#"<a class="z-lightbox-open" href="/p/1.webp" data-zeyn-lightbox data-zeyn-no-prefetch aria-haspopup="dialog" data-caption="From the &quot;old&quot; lighthouse" data-zeyn-lightbox-texts=""#),
        "{out}"
    );
    assert!(out.contains("&quot;close&quot;:&quot;Close&quot;"), "{out}");
    assert!(out.contains(r#"alt="Harbor &lt;dawn&gt;""#));
    assert!(out.ends_with("</a>"), "{out}");
    let tr = html(
        &Image::new("/p/1-small.webp")
            .alt("")
            .size(320, 240)
            .lightbox(),
        "tr",
    );
    assert!(
        tr.contains(r#"aria-label="Resmi aç""#) && tr.contains(r#"href="/p/1-small.webp""#),
        "an image without alt text still names its link, and opens its own file: {tr}"
    );
    let foreign = html(
        &Image::new("/p/1-small.webp")
            .alt("One")
            .size(320, 240)
            .full("https://elsewhere.example/1.webp"),
        "en",
    );
    assert!(
        foreign.contains(r#"href="/p/1-small.webp""#),
        "a larger file on a host the app does not allow is left out: {foreign}"
    );
    let plain = html(
        &Image::new("/p/1-small.webp")
            .alt("One")
            .size(320, 240),
        "en",
    );
    assert!(!plain.contains("z-lightbox"), "{plain}");
}

#[test]
fn a_video_poster_opens_its_file_from_the_media_source() {
    let out = html(
        &Image::new("/p/clip.webp")
            .alt("Harbor clip")
            .size(320, 180)
            .video("films", "harbor.mp4"),
        "en",
    );
    assert!(
        out.contains(r#"href="/_zeyn/media/films/harbor.mp4""#),
        "{out}"
    );
    assert!(out.contains(r#"data-video="/_zeyn/media/films/harbor.mp4""#));
    assert!(out.contains(
        r#"<span class="z-lightbox-play"><span class="z-sr-only">Play video</span></span></a>"#
    ));
    let bad = html(
        &Image::new("/p/clip.webp")
            .alt("Clip")
            .size(320, 180)
            .video("films", "../secret.mp4"),
        "en",
    );
    assert!(
        !bad.contains("z-lightbox"),
        "a file name outside the media source is refused: {bad}"
    );
}

#[tokio::test]
async fn the_lightbox_loads_only_on_pages_with_a_lightbox_image() {
    let router = app();
    let (status, page) = get(&router, "/photos", "social.read").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        links(&page, "lightbox.js") && links(&page, "lightbox.css"),
        "{page}"
    );
    let (_, single) = get(&router, "/photo", "social.read").await;
    assert!(!links(&single, "lightbox.js") && !links(&single, "lightbox.css"));
    let (_, text) = get(&router, "/plain", "social.read").await;
    assert!(!links(&text, "lightbox.js"));
}
