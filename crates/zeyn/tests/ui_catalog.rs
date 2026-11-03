//! Card grids, rails and search boxes: free text search in `DataQuery`, server rendered cards,
//! paging and sorting through the grid endpoint, suggestions through the data endpoint, access,
//! masked fields and rate limits.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity, MAX_SEARCH_QUERY, PageRequest};

#[entity(table = "goods")]
pub struct Good {
    #[key]
    pub id: GoodId,
    pub name: String,
    pub city: String,
    pub price: i64,
}

fn goods() -> Vec<Good> {
    let names = [
        "İSTANBUL tea",
        "Ispanak",
        "Izmir figs",
        "Black <tea>",
        "Coffee",
    ];
    (1..=30)
        .map(|i| Good {
            id: GoodId(i),
            name: format!("{} {i}", names[(i as usize - 1) % names.len()]),
            city: if i % 2 == 0 {
                "Ankara".into()
            } else {
                "Bursa".into()
            },
            price: i * 10,
        })
        .collect()
}

#[action(permission = "shop.read")]
pub async fn search_goods(q: DataQuery) -> Result<Page<Good>> {
    Ok(q.apply(goods()))
}

#[test]
fn free_text_matches_text_fields_ignoring_case_and_turkish_i() {
    let q = |text: &str| DataQuery {
        q: text.into(),
        page: PageRequest::Offset {
            offset: 0,
            limit: 500,
        },
        ..DataQuery::default()
    };
    let names = |text: &str| {
        q(text)
            .apply(goods())
            .items
            .into_iter()
            .map(|g| g.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names("istanbul").len(), 6, "İSTANBUL matches istanbul");
    assert_eq!(names("ıspanak"), names("ISPANAK"), "dotless ı and I match");
    assert_eq!(names("izmir").len(), 6, "Izmir matches izmir");
    assert_eq!(names("ankara").len(), 15, "every text field is searched");
    assert!(names("300").is_empty(), "numbers are not text fields");
    assert_eq!(names("  ").len(), 30, "blank text matches every row");
    assert_eq!(
        names(&"x".repeat(MAX_SEARCH_QUERY + 1)).len(),
        30,
        "text over the cap is ignored"
    );
}

#[screen(route = "/shop", public)]
pub fn shop() -> Screen {
    Screen::new("Shop")
        .body(
            CardGrid::of(search_goods)
                .title("All goods")
                .card(|g: &Good| {
                    Card::image(format!("/img/{}.png", g.id.0), g.name.clone())
                        .text(format!("{} TL", g.price))
                        .badge(Badge::new("New"))
                        .link(results, SearchParams { q: g.name.clone() })
                })
                .sort_option("Cheapest", Good::price, false)
                .sort_option("Dearest", Good::price, true)
                .paging(Paging::infinite(4)),
        )
        .body(
            Rail::of(search_goods)
                .title("Picks")
                .card(|g: &Good| Card::image("javascript:alert(1)", g.name.clone())),
        )
        .body(
            SearchBox::of(search_goods)
                .instant(10)
                .limit(3)
                .results(results),
        )
}

#[screen(route = "/results", public)]
pub fn results(p: SearchParams) -> Screen {
    Screen::new("Results")
        .body(SearchBox::of(search_goods).value(p.q.clone()))
        .body(
            CardGrid::of(search_goods)
                .card(|g: &Good| Card::image("/x.png", g.name.clone()))
                .search(p.q)
                .paging(Paging::pages(5)),
        )
}

#[screen(route = "/picks", public)]
pub fn picks() -> Screen {
    Screen::new("Picks").body(
        Rail::of(search_goods)
            .card(|g: &Good| {
                Card::image("/x.png", g.name.clone())
                    .subtitle(g.city.clone())
                    .corner(Badge::new("Hot <1>"))
                    .badges([Badge::new("Tag a"), Badge::new("Tag b")])
                    .progress(g.price as f32 / 100.0, "Sold")
                    .horizontal()
            })
            .paging(Paging::infinite(2)),
    )
}

#[screen(route = "/rows", public)]
pub fn goods_list() -> Screen {
    Screen::new("Rows")
        .body(
            List::of(search_goods)
                .title("Goods")
                .item(|g: &Good| {
                    zeyn::ui::ListItem::new(g.name.clone())
                        .image(if g.id.0 == 1 {
                            "javascript:alert(1)".to_string()
                        } else {
                            format!("/img/{}.png", g.id.0)
                        })
                        .subtitle(g.city.clone())
                        .meta(format!("{} TL", g.price))
                        .link(results, SearchParams { q: g.name.clone() })
                })
                .sort_option("Cheapest", Good::price, false)
                .paging(Paging::infinite(3)),
        )
        .body(
            List::new()
                .item(zeyn::ui::ListItem::new("Static <one>").meta("now"))
                .item(zeyn::ui::ListItem::new("Static two")),
        )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app(limit: Option<RateLimit>) -> axum::Router {
    let mut app = App::new("Test")
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
        .feature(
            Feature::new("shop")
                .screen(shop)
                .screen(results)
                .screen(picks)
                .screen(goods_list),
        );
    if let Some(l) = limit {
        app = app.write_limit(l);
    }
    app.router().unwrap()
}

async fn send(router: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let status = res.status();
    (
        status,
        String::from_utf8(
            res.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap(),
    )
}

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .header("x-perms", "shop.read")
        .body(Body::empty())
        .unwrap()
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
async fn card_grids_render_their_first_page_as_linked_lazy_images() {
    let (status, page) = send(&app(None), get("/shop")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(r#"<section class="z-cards" data-zeyn-cards data-zeyn-paging="infinite""#),
        "{page}"
    );
    assert!(page.contains(r#"data-zeyn-size="4""#) && page.contains(r#"data-zeyn-next="4""#));
    assert!(page.contains(r#"<h2 class="z-cards-title" id="shop-cards-0-title">All goods</h2>"#));
    assert!(page.contains(
        r#"<img class="z-poster-img" src="/img/1.png" alt="" width="300" height="400" loading="lazy" decoding="async">"#
    ));
    assert!(page.contains(r#"<a class="z-poster-link" href="/_zeyn/route/results?params="#));
    assert!(page.contains(r#"<span class="z-badge z-tone-neutral">New</span>"#));
    assert!(page.contains("Black &lt;tea&gt;"), "row text is escaped");
    assert_eq!(
        page.matches(r#"<li class="z-poster">"#)
            .count(),
        4 + 12,
        "four cards and the rail's twelve"
    );
    assert!(page.contains(r#"data-zeyn-cards-sort><option value="0" data-sort=""#));
    assert!(page.contains(r#"href="/_zeyn/catalog.css?v="#));
    assert!(
        !page.contains("javascript:"),
        "unsafe image addresses are left out"
    );
    assert!(page.contains(r#"<section class="z-rail""#) && page.contains("data-zeyn-rail-next"));
    assert!(page.contains(r#"role="combobox" aria-autocomplete="list" aria-expanded="false""#));
    assert!(page.contains(r#"data-zeyn-results="results""#) && page.contains(r#"maxlength="100""#));
}

#[tokio::test]
async fn more_cards_load_through_the_grid_endpoint_with_the_servers_page_size_and_sorts() {
    let router = app(None);
    let ask = |query: serde_json::Value| {
        post(
            "/_zeyn/grid",
            "shop.read",
            serde_json::json!({ "screen": "shop", "node": "0", "query": query }),
        )
    };
    let (status, body) = send(
        &router,
        ask(serde_json::json!({ "page": { "type": "after", "cursor": "4", "limit": 500 }, "sort": [{ "field": "price", "desc": true }] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = json["rows"].as_str().unwrap();
    assert_eq!(
        rows.matches(r#"<li class="z-poster">"#)
            .count(),
        4,
        "the client's limit is ignored"
    );
    assert!(
        rows.contains(">260 TL<") && !rows.contains(">300 TL<"),
        "the offered sort order applies after the cursor: {rows}"
    );
    let (_, body) = send(
        &router,
        ask(serde_json::json!({ "page": { "type": "after", "limit": 4 }, "sort": [{ "field": "city", "desc": true }], "filters": [{ "field": "city", "contains": "x" }], "q": "nothing" })),
    )
    .await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = json["rows"].as_str().unwrap();
    assert!(
        rows.contains(">10 TL<") && rows.matches("<li").count() == 4,
        "sorts not offered, filters and text from the client are dropped: {rows}"
    );
    let (status, _) = send(
        &router,
        post(
            "/_zeyn/grid",
            "",
            serde_json::json!({ "screen": "shop", "node": "0" }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "reading needs the action's permission"
    );
}

#[tokio::test]
async fn results_screens_search_on_the_server_and_page_by_number() {
    let params = serde_json::json!({ "q": "istanbul" }).to_string();
    let path = format!(
        "/results?params={}",
        params
            .replace('{', "%7B")
            .replace('}', "%7D")
            .replace('"', "%22")
            .replace(':', "%3A")
    );
    let (status, page) = send(&app(None), get(&path)).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(
        page.contains(r#"data-zeyn-paging="pages""#) && page.contains(r#"data-zeyn-total="6""#),
        "{page}"
    );
    assert_eq!(
        page.matches(r#"<li class="z-poster">"#)
            .count(),
        5
    );
    assert!(page.contains(r#"value="istanbul""#));
    assert!(page.contains("data-zeyn-cards-next"));
}

#[tokio::test]
async fn suggestions_come_from_the_action_with_a_capped_text() {
    let router = app(None);
    let ask = |input: serde_json::Value, perms: &str| {
        post(
            "/_zeyn/data",
            perms,
            serde_json::json!({ "screen": "shop", "node": "2", "key": "suggest", "input": input }),
        )
    };
    let (status, body) = send(&router, ask(serde_json::json!("ıstanbul"), "shop.read")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hits: Vec<String> = serde_json::from_str(&body).unwrap();
    assert_eq!(
        hits,
        ["İSTANBUL tea 1", "İSTANBUL tea 6", "İSTANBUL tea 11"],
        "three, as the box asks"
    );
    let (_, body) = send(
        &router,
        ask(
            serde_json::json!(format!("{}tea", " ".repeat(5))),
            "shop.read",
        ),
    )
    .await;
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&body)
            .unwrap()
            .len(),
        3
    );
    let (_, body) = send(
        &router,
        ask(serde_json::json!("z".repeat(10_000)), "shop.read"),
    )
    .await;
    assert_eq!(body, "[]", "long text is cut to the cap and finds nothing");
    let (status, _) = send(&router, ask(serde_json::json!("tea"), "")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn suggestions_count_against_the_write_limit() {
    let router = app(Some(RateLimit::per_user(
        2,
        std::time::Duration::from_secs(60),
    )));
    let ask = || {
        post(
            "/_zeyn/data",
            "shop.read",
            serde_json::json!({ "screen": "shop", "node": "2", "key": "suggest", "input": "tea" }),
        )
    };
    assert_eq!(send(&router, ask()).await.0, StatusCode::OK);
    assert_eq!(send(&router, ask()).await.0, StatusCode::OK);
    assert_eq!(send(&router, ask()).await.0, StatusCode::TOO_MANY_REQUESTS);
}

#[entity(table = "members")]
pub struct Member {
    #[key]
    pub id: MemberId,
    pub name: String,
    #[mask(last4)]
    pub phone: String,
}

#[action(public)]
pub async fn members(q: DataQuery) -> Result<Page<Member>> {
    Ok(q.apply(vec![Member {
        id: MemberId(1),
        name: "Ada".into(),
        phone: "5551234567".into(),
    }]))
}

#[screen(route = "/members", public)]
pub fn member_cards() -> Screen {
    Screen::new("Members")
        .body(CardGrid::of(members).card(|m: &Member| Card::image("/m.png", m.phone.clone())))
}

#[tokio::test]
async fn cards_of_models_with_masked_fields_are_refused_to_users_who_may_not_reveal_them() {
    let router = App::new("Test")
        .auth(|_: &HeaderMap| Some(Identity::user("u")))
        .feature(Feature::new("m").screen(member_cards))
        .router()
        .unwrap();
    let (status, page) = send(
        &router,
        Request::get("/members")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !page.contains("5551234567"),
        "the masked value never reaches the page"
    );
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/grid",
            "",
            serde_json::json!({ "screen": "member_cards", "node": "0" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(!body.contains("5551234567"));
}

#[tokio::test]
async fn the_catalog_script_is_served_without_sign_in() {
    let (status, js) = send(
        &app(None),
        Request::get("/_zeyn/catalog.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("aria-activedescendant"));
    assert!(
        js.contains("window.zeynCsrf()"),
        "public pages come without a CSRF token, so paging and suggestions must ask the runtime for one"
    );
}

#[tokio::test]
async fn results_screens_open_without_params_with_empty_text() {
    let (status, page) = send(&app(None), get("/results")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"data-zeyn-total="30""#));
}

#[tokio::test]
async fn image_cards_lay_out_in_a_row_with_subtitle_corner_badges_and_progress() {
    let (status, page) = send(&app(None), get("/picks")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page.matches(r#"<li class="z-poster z-poster-h">"#)
            .count(),
        2
    );
    assert!(
        page.contains(r#"<span class="z-poster-subtitle">Bursa</span>"#),
        "{page}"
    );
    assert!(page.contains(r#"<span class="z-poster-corner"><span class="z-badge z-tone-neutral">Hot &lt;1&gt;</span></span>"#));
    assert!(page.contains(
        r#"<span class="z-poster-badges"><span class="z-badge z-tone-neutral">Tag a</span>"#
    ));
    assert!(page.contains(r#"role="progressbar" aria-label="Sold" aria-valuemin="0" aria-valuemax="100" aria-valuenow="10" aria-valuetext="10%""#));
    assert!(
        page.contains(r#"aria-valuenow="20""#),
        "each row has its own fraction"
    );
    assert!(page.contains("--z-progress:10%"));
}

#[tokio::test]
async fn lists_render_rows_as_single_links_and_page_through_the_grid_endpoint() {
    let router = app(None);
    let (status, page) = send(&router, get("/rows")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(
        page.contains(
            r#"<section class="z-cards z-cards-rows" data-zeyn-cards data-zeyn-paging="infinite""#
        ),
        "{page}"
    );
    assert!(page.contains(r#"<ul class="z-list" id="goods_list-cards-0-list" data-zeyn-rows>"#));
    assert_eq!(
        page.matches(r#"<li class="z-list-item"><a class="z-list-row" href="#)
            .count(),
        3
    );
    assert!(
        page.contains(r#"<span class="z-list-img"></span>"#),
        "unsafe image addresses are left out"
    );
    assert!(!page.contains("javascript:"));
    assert!(
        page.contains(r#"<img class="z-list-img" src="/img/2.png" alt="" width="48" height="48""#)
    );
    assert!(page.contains(r#"<span class="z-list-meta">10 TL</span>"#));
    assert!(page.contains(r#"<li class="z-list-item"><div class="z-list-row"><span class="z-list-body"><span class="z-list-title">Static &lt;one&gt;</span></span><span class="z-list-meta">now</span></div></li>"#));
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/grid",
            "shop.read",
            serde_json::json!({ "screen": "goods_list", "node": "0", "query": { "page": { "type": "after", "cursor": "3", "limit": 99 } } }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let rows = json["rows"].as_str().unwrap();
    assert_eq!(
        rows.matches(r#"<li class="z-list-item">"#)
            .count(),
        3,
        "the server keeps the page size: {rows}"
    );
    let (status, _) = send(
        &router,
        post(
            "/_zeyn/grid",
            "",
            serde_json::json!({ "screen": "goods_list", "node": "0" }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "reading needs the action's permission"
    );
}
