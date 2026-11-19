//! Builders: rows built as any node with stable keys and separators, virtual lists with a fixed
//! row height and their positions, and `Repeat` over in memory items.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity};

#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
}

#[action(permission = "people.read")]
pub async fn search_people(q: DataQuery) -> Result<Page<Person>> {
    let rows = (1..=50)
        .map(|i| Person {
            id: PersonId(i),
            name: format!("P{i}"),
        })
        .collect();
    Ok(q.apply(rows))
}

#[screen(route = "/people", permission = "people.read")]
pub fn people() -> Screen {
    Screen::new("People")
        .body(
            List::of(search_people)
                .builder(|p: &Person| Badge::new(p.name.clone()))
                .separator(Paragraph::new("·"))
                .paging(Paging::infinite(3)),
        )
        .body(
            List::of(search_people)
                .title("All")
                .builder(|p: &Person| Paragraph::new(p.name.clone()))
                .row_height(36)
                .paging(Paging::virtual_scroll()),
        )
        .body(Row::new().col(Repeat::of(["a", "b"], |s| {
            Form::of(Person::default())
                .field(Person::name)
                .listens(s)
        })))
}

fn app() -> Router {
    App::new("Test")
        .i18n(I18n::new("en"))
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
        .feature(Feature::new("people").screen(people))
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn send(req: Request<Body>) -> (StatusCode, String) {
    let res = app().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn post(path: &str, perms: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn built_rows_carry_their_model_key_and_a_separator_before_each() {
    let (status, page) = send(
        Request::get("/people")
            .header("x-perms", "people.read")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"<li data-zeyn-key="1" class="z-built"><span class="z-badge"#));
    assert!(page.contains(r#"<li class="z-separator" aria-hidden="true"><p class="z-paragraph">·</p></li><li data-zeyn-key="2""#));
    assert!(
        page.contains("/_zeyn/vlist.js?v="),
        "a virtual list loads its module"
    );
    assert!(page.contains(
        r#"data-zeyn-vlist data-zeyn-row-height="36" data-zeyn-block="200" data-zeyn-total="50""#
    ));
    assert!(page.contains(
        r#"<li data-zeyn-key="50" aria-posinset="50" aria-setsize="50" class="z-built">"#
    ));
}

#[tokio::test]
async fn virtual_blocks_load_by_offset_with_their_positions_and_access_checks() {
    let query = json!({ "screen": "people", "node": "1", "query": { "page": { "type": "offset", "offset": 40, "limit": 5000 }, "with_total": true } });
    let (status, body) = send(post("/_zeyn/grid", "people.read", query.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = v["rows"].as_str().unwrap();
    assert!(
        rows.starts_with(r#"<li data-zeyn-key="41" aria-posinset="41" aria-setsize="50""#),
        "{rows}"
    );
    assert_eq!(
        rows.matches("<li").count(),
        10,
        "the server sets the block size"
    );
    let (status, _) = send(post("/_zeyn/grid", "", query)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn repeated_nodes_have_nested_paths_and_refresh_through_them() {
    let body = json!({ "screen": "people", "nodes": ["2.0.1"] });
    let (status, text) = send(post("/_zeyn/nodes", "people.read", body)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert!(text.contains(r#"data-zeyn-listens=\"b\""#), "{text}");
}

#[test]
fn the_page_check_asks_for_a_row_height_and_refuses_separators_in_virtual_lists() {
    let app = App::new("T").feature(Feature::new("t").screen(people));
    assert!(app.check_page(&people::run()).is_ok());
    let bad = Screen::new("Bad").body(
        List::of(search_people)
            .builder(|p: &Person| Paragraph::new(p.name.clone()))
            .separator(Paragraph::new("-"))
            .paging(Paging::virtual_scroll()),
    );
    let problems = app
        .check_page(&bad)
        .unwrap_err()
        .join("\n");
    assert!(problems.contains("needs `.row_height(px)`"), "{problems}");
    assert!(
        problems.contains("`separator` does not work with virtual scrolling"),
        "{problems}"
    );
}
