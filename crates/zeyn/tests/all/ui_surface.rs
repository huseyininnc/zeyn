//! Surface: free placement and resizing of items inside bounds, on a snap grid, on an axis and
//! inside size limits and an aspect ratio, checked again on the server before the move action
//! runs.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::Axis;
use zeyn::{I18n, Identity};

#[derive(Clone, Default)]
pub struct Moves(Arc<Mutex<Vec<SurfaceMove>>>);

impl Service for Moves {}

#[action(permission = "floor.edit")]
pub async fn move_table(m: SurfaceMove, moves: Moves) -> Result<()> {
    if m.id == "locked" {
        return Err(Error::Forbidden);
    }
    moves.0.lock().unwrap().push(m);
    Ok(())
}

#[screen(route = "/floor", permission = "floor.read")]
pub fn floor() -> Screen {
    Screen::new("Floor").body(
        Card::new().add(
            Surface::new("Floor plan", 400, 300)
                .snap(20)
                .on_move(move_table)
                .item(
                    Draggable::new("t1", Paragraph::new("Table <1>"))
                        .label("Table <1>")
                        .at(40, 60)
                        .size(100, 50),
                )
                .item(
                    Draggable::new("rail", Paragraph::new("Rail"))
                        .at(0, 200)
                        .size(80, 40)
                        .axis(Axis::X),
                )
                .item(Draggable::new("locked", Paragraph::new("Locked")))
                .item(
                    Draggable::new("note", Paragraph::new("Note"))
                        .at(200, 100)
                        .size(100, 60)
                        .resizable()
                        .min_size(60, 40),
                )
                .item(
                    Draggable::new("photo", Paragraph::new("Photo"))
                        .size(160, 90)
                        .min_size(80, 40)
                        .max_size(320, 320)
                        .aspect(16, 9),
                ),
        ),
    )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app(moves: Moves) -> axum::Router {
    App::new("Surface")
        .i18n(I18n::new("en"))
        .service(moves)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("floor").screen(floor))
        .router()
        .unwrap()
}

async fn send(router: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
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

async fn place(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    input: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let call = serde_json::json!({
        "screen": "floor",
        "params": null,
        "node": "0.0",
        "key": "move",
        "input": input,
    });
    send(
        router,
        req.body(Body::from(call.to_string()))
            .unwrap(),
    )
    .await
}

const EDIT: &str = "floor.read,floor.edit";

#[tokio::test]
async fn a_surface_renders_its_items_in_place_with_escaped_labels_and_its_modules() {
    let router = app(Moves::default());
    let (status, page) = send(
        &router,
        Request::get("/floor")
            .header("x-perms", "floor.read")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(r#"data-zeyn-surface="floor-surface-0-0" data-snap="20" data-save "#));
    assert!(page.contains("--z-surface-w:400px;--z-surface-h:300px"));
    assert!(page.contains(r#"aria-label="Table &lt;1&gt;" aria-describedby="floor-surface-0-0-hint" data-id="t1" data-axis="both" style="left:40px;top:60px;width:100px;height:50px""#));
    assert!(page.contains(r#"data-id="rail" data-axis="x" style"#));
    assert!(page.contains(r#"data-id="note" data-axis="both" data-min="60,40" data-max="20000,20000" style="left:200px;top:100px;width:100px;height:60px""#));
    assert!(page.contains(r#"data-min="80,40" data-max="320,320" data-aspect="16,9" style="left:0px;top:0px;width:160px;height:90px""#));
    assert_eq!(
        page.matches(r#"class="z-surface-resize""#)
            .count(),
        2
    );
    assert!(page.contains(r#"<div class="z-node" data-zeyn-node="0.0.0">"#));
    for name in ["surface.js", "surface.css", "drag.js", "net.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
}

#[tokio::test]
async fn the_server_keeps_a_move_inside_the_bounds_on_the_grid_and_on_the_axis() {
    let moves = Moves::default();
    let router = app(moves.clone());
    let (status, body) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "t1", "x": 131, "y": -40 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(r#""x":140"#) && body.contains(r#""y":0"#),
        "{body}"
    );
    let (_, body) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "t1", "x": 9999, "y": 9999 }),
    )
    .await;
    assert!(
        body.contains(r#""x":300"#) && body.contains(r#""y":240"#),
        "{body}"
    );
    let (_, body) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "rail", "x": 75, "y": 10 }),
    )
    .await;
    assert!(
        body.contains(r#""x":80"#) && body.contains(r#""y":200"#),
        "{body}"
    );
    let stored = moves.0.lock().unwrap().clone();
    assert_eq!(
        stored,
        vec![
            SurfaceMove {
                id: "t1".into(),
                x: 140,
                y: 0,
                w: 100,
                h: 50,
            },
            SurfaceMove {
                id: "t1".into(),
                x: 300,
                y: 240,
                w: 100,
                h: 50,
            },
            SurfaceMove {
                id: "rail".into(),
                x: 80,
                y: 200,
                w: 80,
                h: 40,
            },
        ]
    );
}

#[tokio::test]
async fn a_move_needs_csrf_the_permission_and_an_item_of_the_surface() {
    let moves = Moves::default();
    let router = app(moves.clone());
    let ok = serde_json::json!({ "id": "t1", "x": 20, "y": 20 });
    let (status, _) = place(&router, EDIT, false, ok.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no CSRF header");
    let (status, _) = place(&router, "floor.read", true, ok).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no permission of the action");
    let (status, _) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "t9", "x": 0, "y": 0 }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an id the screen never gave");
    let (status, _) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "t1", "x": "a", "y": 0 }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a position that is not a number"
    );
    let (status, _) = place(
        &router,
        EDIT,
        true,
        serde_json::json!({ "id": "locked", "x": 0, "y": 0 }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the action refuses");
    assert!(moves.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_server_keeps_a_resize_inside_the_limits_the_bounds_the_grid_and_the_aspect_ratio() {
    let moves = Moves::default();
    let router = app(moves.clone());
    let cases = [
        (
            serde_json::json!({ "id": "note", "x": 200, "y": 100, "w": 133, "h": 7 }),
            (200, 100, 140, 40),
        ),
        (
            serde_json::json!({ "id": "note", "x": 200, "y": 100, "w": 9999, "h": 9999 }),
            (0, 0, 400, 300),
        ),
        (
            serde_json::json!({ "id": "photo", "x": 0, "y": 0, "w": 250, "h": 1 }),
            (0, 0, 260, 146),
        ),
        (
            serde_json::json!({ "id": "photo", "x": 0, "y": 0, "w": 9999, "h": 9999 }),
            (0, 0, 320, 180),
        ),
        (
            serde_json::json!({ "id": "t1", "x": 40, "y": 60, "w": 300, "h": 300 }),
            (40, 60, 100, 50),
        ),
    ];
    for (input, (x, y, w, h)) in cases {
        let (status, body) = place(&router, EDIT, true, input.clone()).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let got: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            got,
            serde_json::json!({ "x": x, "y": y, "w": w, "h": h }),
            "{input}"
        );
    }
    let stored = moves.0.lock().unwrap().clone();
    assert_eq!(stored.len(), 5);
    assert_eq!(
        stored[2],
        SurfaceMove {
            id: "photo".into(),
            x: 0,
            y: 0,
            w: 260,
            h: 146,
        }
    );
}
