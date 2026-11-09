//! The social pack: story rings, the lightbox and the feed.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{MAX_STORY_RINGS, ReactionPress, ReactionState};
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

/// A post of the feed tests: posts with an odd author are private and only their author's
/// action call would return them, so the feed scopes them out below.
#[entity(table = "notes")]
pub struct Note {
    #[key]
    pub id: NoteId,
    pub author: String,
    pub text: String,
    pub public: bool,
}

fn notes() -> Vec<Note> {
    (1..=30)
        .rev()
        .map(|i| Note {
            id: NoteId(i),
            author: format!("Author <{i}>"),
            text: format!("Text {i} <script>alert(1)</script>\nsecond line"),
            public: i % 2 == 0,
        })
        .collect()
}

/// Notes newest first, after the cursor (the id of the last note shown), with the filters.
#[action(permission = "social.read")]
pub async fn search_notes(q: DataQuery) -> Result<Page<Note>> {
    let below = match &q.page {
        zeyn::PageRequest::After {
            cursor: Some(c), ..
        } => c.0.parse::<i64>().unwrap_or(i64::MAX),
        _ => i64::MAX,
    };
    let limit = match &q.page {
        zeyn::PageRequest::After { limit, .. } | zeyn::PageRequest::Offset { limit, .. } => *limit,
    };
    let mut filtered = DataQuery {
        page: zeyn::PageRequest::Offset {
            offset: 0,
            limit: 1000,
        },
        filters: q.filters.clone(),
        ..DataQuery::default()
    }
    .apply(notes())
    .items;
    filtered.sort_by_key(|n| std::cmp::Reverse(n.id.0));
    let items: Vec<Note> = filtered
        .into_iter()
        .filter(|n| n.id.0 < below)
        .take(limit as usize)
        .collect();
    let mut page = Page::new(items);
    page.next = page
        .items
        .last()
        .filter(|n| n.id.0 > 2)
        .map(|n| zeyn::Cursor(n.id.0.to_string()));
    Ok(page)
}

/// Echoes a press: the post's key in the count name, so tests see what the action got.
#[action(permission = "social.react")]
pub async fn react_to_note(press: ReactionPress) -> Result<ReactionState> {
    Ok(ReactionState::default().count(format!("{}:{}", press.target, press.reaction), 1))
}

fn note_feed() -> Feed<Note> {
    Feed::of(search_notes)
        .post(|n: &Note| {
            Post::new(n.author.clone(), DateTime::from_unix_ms(1_778_580_000_000))
                .text(n.text.clone())
                .media(
                    Image::new(format!("/n/{}.webp", n.id.0))
                        .alt("Photo")
                        .size(300, 200),
                )
                .reactions(ReactionState::default().count("like", 3))
                .comments(1)
                .link(person_stories, PersonParams { id: n.id.0 })
        })
        .batch(5)
        .scope(Note::public, true)
        .reactions(react_to_note)
        .reaction("like", "Like", "+")
        .topic("notes")
}

#[screen(route = "/feed", permission = "social.read")]
pub fn feed_screen() -> Screen {
    Screen::new("Feed").body(Stack::new().add(note_feed().title("Notes")))
}

fn feature() -> Feature {
    Feature::new("social")
        .screen(feed_screen)
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
        .write_limit(zeyn::RateLimit::per_user(
            8,
            std::time::Duration::from_secs(60),
        ))
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

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn post_json(
    router: &axum::Router,
    path: &str,
    perms: &str,
    csrf: bool,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = router
        .clone()
        .oneshot(
            req.body(Body::from(body.to_string()))
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// A reaction press on the post with `key`, through the feed at the nested path `0.0`.
fn press(key: &str, reaction: &str) -> serde_json::Value {
    serde_json::json!({ "screen": "feed_screen", "params": null, "node": "0.0", "key": "react", "input": { "key": key, "reaction": reaction, "pressed": true } })
}

#[tokio::test]
async fn a_feed_is_an_apg_feed_of_escaped_posts() {
    let router = app();
    let (status, page) = get(&router, "/feed", "social.read").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(r#"<section class="z-feed" data-zeyn-feed="notes" data-zeyn-next="22""#),
        "{page}"
    );
    assert!(page.contains(r#"role="feed" aria-labelledby="#), "{page}");
    assert!(
        page.contains(r#"<article class="z-post" tabindex="0" data-zeyn-key="30" aria-posinset="1" aria-setsize="-1" aria-labelledby="#),
        "{page}"
    );
    assert_eq!(page.matches("<article").count(), 5, "five a batch");
    assert!(
        !page.contains(r#"data-zeyn-key="29""#),
        "the odd ones are scoped out"
    );
    assert!(
        page.contains("Author &lt;30&gt;") && !page.contains("<script>alert"),
        "text is escaped"
    );
    assert!(page.contains(r#"data-zeyn-gallery"#) && page.contains(r#"data-zeyn-lightbox "#));
    assert!(
        page.contains(r#"fetchpriority="high""#),
        "the first image of the first post loads at once"
    );
    assert!(page.contains(r#"data-zeyn-reactions data-zeyn-post"#));
    assert!(page.contains(r#"<span class="z-sr-only">1 comment</span>"#));
    assert!(page.contains("data-zeyn-share"));
    for name in ["feed.js", "feed.css", "lightbox.js", "reactions.js"] {
        assert!(links(&page, name), "{name}");
    }
    let (_, text) = get(&router, "/plain", "social.read").await;
    assert!(!links(&text, "feed.js") && !links(&text, "feed.css"));
    let (status, _) = get(&router, "/feed", "other").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn older_posts_load_by_cursor_with_the_servers_batch_and_scope() {
    let router = app();
    let body = serde_json::json!({ "screen": "feed_screen", "params": null, "node": "0.0", "query": { "page": { "type": "after", "cursor": "26", "limit": 500 }, "filters": [{ "field": "public", "is": false }] } });
    let (status, json) = post_json(&router, "/_zeyn/grid", "social.read", true, body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let rows = json["rows"].as_str().unwrap();
    assert_eq!(
        rows.matches("<article").count(),
        5,
        "the batch of the server: {rows}"
    );
    assert!(rows.contains(r#"data-zeyn-key="24""#) && !rows.contains(r#"data-zeyn-key="25""#));
    let (status, _) = post_json(&router, "/_zeyn/grid", "social.read", false, body.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the CSRF check");
    let flat =
        serde_json::json!({ "screen": "feed_screen", "params": null, "node": 0, "query": {} });
    let (status, _) = post_json(&router, "/_zeyn/grid", "social.read", true, flat).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the feed is at the nested path 0.0"
    );
}

#[tokio::test]
async fn a_reaction_reaches_the_action_only_for_a_post_the_user_can_read() {
    let router = app();
    let both = "social.read,social.react";
    let (status, state) = post_json(&router, "/_zeyn/data", both, true, press("28", "like")).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(
        state["counts"]["28:like"], 1,
        "the post key reaches the action: {state}"
    );
    let (status, _) = post_json(&router, "/_zeyn/data", both, true, press("27", "like")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a post outside the feed's scope"
    );
    let (status, _) = post_json(&router, "/_zeyn/data", both, true, press("999", "like")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a post that does not exist");
    let (status, _) = post_json(&router, "/_zeyn/data", both, true, press("28", "angry")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a reaction outside the set");
    let long = "9".repeat(200);
    let (status, _) = post_json(&router, "/_zeyn/data", both, true, press(&long, "like")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a key longer than any post key"
    );
    let (status, _) = post_json(
        &router,
        "/_zeyn/data",
        "social.read",
        true,
        press("28", "like"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the reaction action's permission"
    );
    let (status, _) = post_json(&router, "/_zeyn/data", both, false, press("28", "like")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the CSRF check");
    let mut last = StatusCode::OK;
    for _ in 0..8 {
        last = post_json(&router, "/_zeyn/data", both, true, press("28", "like"))
            .await
            .0;
    }
    assert_eq!(
        last,
        StatusCode::TOO_MANY_REQUESTS,
        "presses count against the write limit"
    );
}

#[test]
fn a_feed_without_posts_or_reactions_is_reported() {
    let mut check = zeyn::ui::PageCheck::default();
    Feed::of(search_notes)
        .reactions(react_to_note)
        .page_check(&mut check);
    let problems = format!("{check:?}");
    assert!(problems.contains("needs `.post("), "{problems}");
    assert!(problems.contains("offers nothing to press"), "{problems}");
}
