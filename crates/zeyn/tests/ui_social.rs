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
pub fn plain() -> Screen {
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

fn feature() -> Feature {
    Feature::new("social")
        .screen(person_stories)
        .screen(plain)
        .screen(stories)
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
