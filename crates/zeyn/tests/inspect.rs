//! Source inspector: debug builds remember the Rust line that placed each node, tag rendered
//! nodes with it when the app is served, link the inspector module only then, and describe the
//! node tree for `zeyn_inspect`. Release builds keep none of it.
//!
//! The source tags are a switch for the whole process, so this file holds one test that turns it
//! on and off in order.

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;

#[screen(route = "/", public)]
pub fn home() -> Screen {
    Screen::new("Home").body(
        Stack::new()
            .add(Heading::h2("Hello"))
            .add(Badge::new("New")),
    )
}

/// The line of `.add(Badge::new("New"))` above.
const BADGE_LINE: u32 = 19;

fn app() -> App {
    App::new("Inspect").feature(Feature::new("inspect").screen(home))
}

async fn page() -> String {
    let res = app()
        .router()
        .unwrap()
        .oneshot(
            Request::get("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn a_node_is_one_boxed_pointer_in_release_and_carries_its_line_only_in_debug() {
    let words = if cfg!(debug_assertions) { 3 } else { 2 };
    assert_eq!(
        std::mem::size_of::<Node>(),
        words * std::mem::size_of::<usize>()
    );
    let screen = home::run();
    let badge = &screen.nodes()[0].children()[1];
    match cfg!(debug_assertions) {
        true => {
            let at = badge.placed_at().unwrap();
            assert!(at.ends_with(&format!("inspect.rs:{BADGE_LINE}:14")), "{at}");
        }
        false => assert_eq!(badge.placed_at(), None),
    }
}

#[tokio::test]
async fn served_pages_tag_nodes_and_link_the_inspector_only_in_debug() {
    let plain = page().await;
    assert!(!plain.contains("data-zeyn-src"));
    assert!(!plain.contains("/_zeyn/inspect.js"));

    Node::tag_sources(true);
    let tagged = page().await;
    let inspector = app()
        .router()
        .unwrap()
        .oneshot(
            Request::get("/_zeyn/inspect.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status();
    Node::tag_sources(false);

    match cfg!(debug_assertions) {
        true => {
            let line = format!(":{BADGE_LINE}:14\"");
            assert!(tagged.contains("data-zeyn-src=\""), "{tagged}");
            assert!(tagged.contains(&line), "{tagged}");
            assert!(tagged.contains("/_zeyn/inspect.js"));
            assert_eq!(inspector, 200);
        }
        false => {
            assert!(!tagged.contains("data-zeyn-src"));
            assert!(!tagged.contains("/_zeyn/inspect.js"));
            assert_eq!(inspector, 404);
        }
    }
}

#[tokio::test]
async fn inspect_returns_the_node_tree_with_paths_types_and_lines() {
    let tree = app().inspect("home").await;
    assert_eq!(tree["screen"], "home");
    let stack = &tree["nodes"][0];
    assert_eq!(stack["path"], "0");
    assert_eq!(stack["children"][1]["path"], "0.1");
    assert_eq!(stack["children"][1]["type"], "badge");
    let source = stack["children"][1]["source"].as_str();
    match cfg!(debug_assertions) {
        true => assert!(
            source
                .unwrap()
                .ends_with(&format!(":{BADGE_LINE}:14"))
        ),
        false => assert_eq!(source, None),
    }
    let missing = app().inspect("nope").await;
    assert_eq!(missing["screens"][0], "home");
    assert!(
        missing["error"]
            .as_str()
            .unwrap()
            .contains("nope")
    );
}
