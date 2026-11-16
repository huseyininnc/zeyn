//! Drives the app in memory with `zeyn::testing::TestApp`: the same router, checks and access
//! rules as the served app, without opening a port. See "Testing" in the Zeyn docs.

#![allow(clippy::unwrap_used)]

#[path = "../src/app.rs"]
mod app;
#[path = "../src/features/mod.rs"]
mod features;

use zeyn::testing::{TestApp, json};
use zeyn::{Config, Identity, StaticSource};

async fn config() -> Config {
    Config::load(vec![Box::new(StaticSource(Default::default()))])
        .await
        .unwrap()
}

/// The app with its own sign in (`DevAuth` in debug builds).
async fn test_app() -> TestApp {
    TestApp::new(app::app(config().await).unwrap())
}

#[tokio::test]
async fn the_app_has_no_structural_problems() {
    assert_eq!(
        app::app(config().await)
            .unwrap()
            .check(),
        Ok(())
    );
}

#[tokio::test]
async fn the_notes_list_opens() {
    let page = test_app().await.get("/notes/").await;
    assert_eq!(page.status, 200);
    assert!(page.body.contains("data-zeyn-grid"));
}

#[tokio::test]
async fn saving_through_the_api_validates_the_title() {
    let note =
        json!({ "id": 0, "title": "", "body": null, "due": null, "done": false, "version": 0 });
    let res = test_app()
        .await
        .api("notes", "save_note", note)
        .await;
    assert_eq!(res.status, 422);
}

#[tokio::test]
async fn the_editor_saves_a_note_and_the_list_shows_it() {
    let app = test_app().await;
    let saved = app
        .submit(
            "note_editor",
            json!({ "id": null }),
            "0",
            json!({ "title": "Call Ada" }),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.effects(), ["toast", "close_and_refresh"]);
    let rows = app
        .grid("note_list", json!(null), "0", json!({}))
        .await;
    assert!(rows.body.contains("Call Ada"), "{}", rows.body);
}

#[tokio::test]
async fn a_user_without_permissions_cannot_open_the_notes() {
    let app = TestApp::signed_in(app::app(config().await).unwrap(), Identity::user("guest"));
    assert_eq!(app.get("/notes/").await.status, 403);
}
