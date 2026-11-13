//! DevTools of debug builds: each request records its screen, action, build and render times,
//! SQL statements, effects and topics; `GET /_zeyn/devtools` lists them and full pages link the
//! panel script. Release builds have none of it.
//!
//! The DevTools are a switch for the whole process, so this file holds one test that turns it on
//! and off in order.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;

#[entity(table = "notes")]
pub struct Note {
    #[key]
    pub id: NoteId,
    #[rule(required)]
    pub text: String,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Note>>>);

impl Service for Store {}

#[action(public)]
pub async fn save_note(n: Valid<Note>, store: Store) -> Result<Note> {
    let note = n.into_inner();
    store
        .0
        .write()
        .unwrap()
        .push(note.clone());
    Ok(note)
}

#[screen(route = "/notes", public)]
pub fn notes() -> Screen {
    Screen::new("Notes").body(
        Form::of(Note {
            id: NoteId(1),
            text: String::new(),
        })
        .field(Note::text)
        .submit("Save", save_note)
        .on_success(|_, ui| {
            ui.toast("Saved");
            ui.refresh(["notes"]);
        }),
    )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn router() -> Router {
    App::new("DevTools")
        .service(Store::default())
        .feature(
            Feature::new("notes")
                .screen(notes)
                .api(save_note),
        )
        .router()
        .unwrap()
}

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(req)
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

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .body(Body::empty())
        .unwrap()
}

fn submit() -> Request<Body> {
    let body = serde_json::json!({ "screen": "notes", "node": 0, "values": { "text": "Hello" } });
    Request::post("/_zeyn/form")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[cfg(debug_assertions)]
#[tokio::test]
async fn requests_are_recorded_only_while_the_devtools_are_on() {
    let router = router();
    let (_, page) = send(&router, get("/notes")).await;
    assert!(!page.contains("/_zeyn/devtools.js"));
    let (status, _) = send(&router, get("/_zeyn/devtools")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    zeyn::server::devtools(true);
    let (_, page) = send(&router, get("/notes")).await;
    let (saved, body) = send(&router, submit()).await;
    let (status, log) = send(&router, get("/_zeyn/devtools")).await;
    let (_, script) = send(&router, get("/_zeyn/devtools.js")).await;
    zeyn::server::devtools(false);

    assert!(page.contains("/_zeyn/devtools.js"), "{page}");
    assert!(script.contains("Alt+Shift+D"));
    assert_eq!(saved, StatusCode::OK, "{body}");
    assert_eq!(status, StatusCode::OK);
    let log: serde_json::Value = serde_json::from_str(&log).unwrap();
    let requests = log["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 2, "{log}");

    let form = &requests[0];
    assert_eq!(form["path"], "/_zeyn/form");
    assert_eq!(form["kind"], "form");
    assert_eq!(form["screen"], "notes");
    assert_eq!(form["action"], "save_note");
    assert_eq!(form["ok"], true);
    assert_eq!(form["effects"], serde_json::json!(["toast", "refresh"]));
    assert_eq!(form["topics"], serde_json::json!(["notes"]));

    let page = &requests[1];
    assert_eq!(page["path"], "/notes");
    assert_eq!(page["kind"], "page");
    assert_eq!(page["status"], 200);
    assert!(page["build_ms"].as_f64().is_some());
    assert!(page["render_ms"].as_f64().is_some());
    assert_eq!(page["query_count"], 0);

    let (status, _) = send(&router, get("/_zeyn/devtools")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[cfg(not(debug_assertions))]
#[tokio::test]
async fn release_builds_have_no_devtools() {
    let router = router();
    let (_, page) = send(&router, get("/notes")).await;
    assert!(!page.contains("devtools"));
    for path in ["/_zeyn/devtools", "/_zeyn/devtools.js"] {
        let (status, _) = send(&router, get(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}
