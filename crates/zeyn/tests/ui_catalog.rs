//! Card grids, rails and search boxes: free text search in `DataQuery`, server rendered cards,
//! paging and sorting through the grid endpoint, suggestions through the data endpoint, access,
//! masked fields and rate limits.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{CenterOnPhone, I18n, Identity, MAX_SEARCH_QUERY, PageRequest};

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
                .sort_option("Cheapest", Good::price)
                .sort_option_desc("Dearest", Good::price)
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

#[params]
pub struct GoodParams {
    pub id: i64,
}

/// A screen with a path route, so card links to it are canonical paths.
#[screen(route = "/goods/{id}", public)]
pub fn good_page(p: GoodParams) -> Screen {
    Screen::new(format!("Good {}", p.id))
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
                    .link(good_page, GoodParams { id: g.id.0 })
            })
            .scrollbar()
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
                .sort_option("Cheapest", Good::price)
                .paging(Paging::infinite(3)),
        )
        .body(
            List::new()
                .item(zeyn::ui::ListItem::new("Static <one>").meta("now"))
                .item(zeyn::ui::ListItem::new("Static two")),
        )
}

/// Fourteen rows paged by the list without a total, as an action that does not count.
#[action(permission = "shop.read")]
pub async fn uncounted_goods(q: DataQuery) -> Result<Page<Good>> {
    let all: Vec<Good> = goods().into_iter().take(14).collect();
    let mut page = Page::from_slice(&all, &q);
    page.total = None;
    Ok(page)
}

#[screen(route = "/uncounted", public)]
pub fn uncounted_list() -> Screen {
    Screen::new("Uncounted").body(
        List::of(uncounted_goods)
            .item(|g: &Good| {
                zeyn::ui::ListItem::new(g.name.clone()).meta(zeyn::ui::RelativeTime::new(
                    DateTime::from_unix_ms(1_778_580_000_000),
                ))
            })
            .paging(Paging::pages(10)),
    )
}

#[screen(route = "/ankara", public)]
pub fn ankara_goods() -> Screen {
    Screen::new("Ankara")
        .body(
            Rail::of(search_goods)
                .card(|g: &Good| Card::image("/x.png", format!("{} in {}", g.name, g.city)))
                .scope(Good::city, "ankara".to_string())
                .paging(Paging::infinite(4)),
        )
        .body(
            List::of(search_goods)
                .item(|g: &Good| zeyn::ui::ListItem::new(format!("{} in {}", g.name, g.city)))
                .scope(Good::city, "Ankara".to_string())
                .paging(Paging::pages(4)),
        )
}

/// A product grid with placeholders, low resolution files, fallbacks and priority cards.
#[screen(route = "/fast", public)]
pub fn fast_goods() -> Screen {
    Screen::new("Fast").body(
        CardGrid::of(search_goods)
            .card(|g: &Good| {
                let card = Card::image(format!("/img/{}.webp", g.id.0), g.name.clone())
                    .low(format!("/img/{}-40.webp", g.id.0))
                    .fallback("/img/none.webp");
                match g.id.0 {
                    3 => card.placeholder(Placeholder::color("--z-surface")),
                    4 => card.low("https://other.test/x.webp"),
                    _ => card,
                }
            })
            .placeholder(Placeholder::color("#d9dde3"))
            .priority(2)
            .paging(Paging::infinite(6)),
    )
}

#[tokio::test]
async fn product_cards_reserve_their_box_show_a_placeholder_and_load_only_the_first_cards_first() {
    let router = app(None);
    let (status, page) = send(&router, get("/fast")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(
        r#"<span class="z-poster-media z-img-host" style="aspect-ratio:300 / 400;--z-ph:#d9dde3"><img class="z-img-under" src="/img/1-40.webp" alt="" decoding="async" fetchpriority="high"><img class="z-poster-img" src="/img/1.webp" alt="" width="300" height="400" loading="eager" fetchpriority="high" data-zeyn-img data-zeyn-fallback="/img/none.webp" decoding="async" draggable="false">"#
    ), "{page}");
    assert_eq!(
        page.matches(r#"fetchpriority="high" data-zeyn-img"#)
            .count(),
        2,
        "only the first two cards come first"
    );
    assert!(page.contains(
        r#"<img class="z-poster-img" src="/img/3.webp" alt="" width="300" height="400" loading="lazy" data-zeyn-img"#
    ));
    assert!(
        page.contains(r#"style="aspect-ratio:300 / 400;--z-ph:var(--z-surface)""#),
        "a card's own placeholder wins over the grid's"
    );
    assert!(
        !page.contains("other.test"),
        "a low file outside img-src is left out"
    );
    assert!(
        page.contains(r#"<li data-zeyn-key="1" class="z-poster">"#),
        "card grid cards carry their model key, so a refresh of a windowed grid finds the focused card"
    );
    assert!(
        page.contains(r#"style="--z-card-ratio:1.333""#),
        "the section carries the image ratio that sizes off screen cards"
    );
    let linked = |name: &str| page.contains(&format!("/_zeyn/{name}?v="));
    assert!(linked("media.js") && linked("media.css"));
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/grid",
            "shop.read",
            serde_json::json!({ "screen": "fast_goods", "node": "0", "query": { "page": { "type": "after", "cursor": "6", "limit": 6 } } }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("data-zeyn-img") && !body.contains("fetchpriority"),
        "pages loaded later never come first: {body}"
    );
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
                .screen(good_page)
                .screen(goods_list)
                .screen(uncounted_list)
                .screen(ankara_goods)
                .screen(fast_goods),
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
        r#"<img class="z-poster-img" src="/img/1.png" alt="" width="300" height="400" loading="lazy" decoding="async" draggable="false">"#
    ));
    assert!(page.contains(r#"<a class="z-poster-link" href="/results?params="#));
    assert!(page.contains(r#"<span class="z-badge z-tone-neutral">New</span>"#));
    assert!(page.contains("Black &lt;tea&gt;"), "row text is escaped");
    assert_eq!(
        page.matches(r#"class="z-poster">"#)
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
        rows.matches(r#"class="z-poster">"#)
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
        page.matches(r#"class="z-poster">"#)
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
async fn paged_lists_without_a_total_offer_the_next_page_while_there_is_a_cursor() {
    let router = app(None);
    let (status, page) = send(&router, get("/uncounted")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"data-zeyn-paging="pages""#), "{page}");
    assert!(page.contains(r#" data-zeyn-next="10""#), "{page}");
    assert!(
        !page.contains("data-zeyn-total"),
        "the action did not count"
    );
    assert!(
        page.contains(r#"<span class="z-cards-page" data-zeyn-cards-page>Page 1</span>"#),
        "without a total the label leaves out the page count: {page}"
    );
    assert!(
        page.contains(r#"data-zeyn-cards-next aria-disabled="false""#),
        "Next works while there is a cursor: {page}"
    );
    assert!(page.contains(r#"data-text-page-only="Page {page}""#));
    assert!(
        page.contains(r#"<span class="z-list-meta"><time"#),
        "the meta of a row takes an inline component: {page}"
    );
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/grid",
            "shop.read",
            serde_json::json!({ "screen": "uncounted_list", "node": "0", "query": { "page": { "type": "offset", "offset": 10, "limit": 10 }, "with_total": true } }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["rows"]
            .as_str()
            .unwrap()
            .matches(r#"<li class="z-list-item">"#)
            .count(),
        4,
        "page 2 holds the last four rows: {body}"
    );
    assert!(json["next"].is_null(), "{body}");
    let (_, js) = send(
        &router,
        Request::get("/_zeyn/catalog.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        js.contains("known ? offset + size >= total : !d.zeynNext"),
        "the browser enables Next from the cursor when there is no total"
    );
}

#[tokio::test]
async fn scoped_rails_and_lists_keep_their_filter_whatever_the_client_sends() {
    let router = app(None);
    let (status, page) = send(&router, get("/ankara")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page.matches(" in Ankara").count(),
        8,
        "four cards and four rows of the scope: {page}"
    );
    assert!(!page.contains(" in Bursa"), "{page}");
    for node in ["0", "1"] {
        for filters in [
            serde_json::json!([]),
            serde_json::json!([{ "field": "city", "equals": "Bursa" }]),
            serde_json::json!([{ "field": "id", "range": { "from": "1", "to": "30" } }]),
        ] {
            let (status, body) = send(
                &router,
                post(
                    "/_zeyn/grid",
                    "shop.read",
                    serde_json::json!({ "screen": "ankara_goods", "node": node, "query": { "page": { "type": "offset", "offset": 4, "limit": 99 }, "filters": filters } }),
                ),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            let rows = json["rows"].as_str().unwrap();
            assert!(
                rows.contains(" in Ankara") && !rows.contains(" in Bursa"),
                "node {node} with {filters}: {rows}"
            );
        }
    }
}

#[tokio::test]
async fn rails_hide_their_scrollbar_unless_kept_and_drag_only_with_a_mouse() {
    let router = app(None);
    let (_, shop_page) = send(&router, get("/shop")).await;
    assert!(
        shop_page.contains(r#"<section class="z-rail" "#),
        "{shop_page}"
    );
    let (_, picks_page) = send(&router, get("/picks")).await;
    assert!(
        picks_page.contains(r#"<section class="z-rail z-rail-bar" "#),
        "{picks_page}"
    );
    assert!(
        picks_page.contains("data-zeyn-rail-prev") && picks_page.contains("data-zeyn-rail-next"),
        "the buttons cannot be turned off"
    );
    let asset = |path: &'static str| {
        let router = router.clone();
        async move {
            send(
                &router,
                Request::get(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .1
        }
    };
    let css = asset("/_zeyn/catalog.css").await;
    assert!(
        css.contains("scrollbar-width: none")
            && css.contains("::-webkit-scrollbar { display: none; }")
    );
    assert!(css.contains("scroll-snap-type: x proximity"));
    assert!(
        css.contains(".z-rail:not([data-zeyn-ready]) .z-rail-nav { visibility: hidden; }"),
        "without the script the buttons are hidden but keep their space"
    );
    let js = asset("/_zeyn/catalog.js").await;
    assert!(
        js.contains("e.pointerType !== 'mouse'"),
        "touch and pen keep native scrolling"
    );
    assert!(js.contains("const DRAG_PX = 6;") && js.contains("D.press(e, {"));
    assert!(
        js.contains("if (scrolled && entry.isIntersecting)"),
        "more cards load by themselves only after the user scrolled, so the first page does not shift"
    );
    let shell = asset("/_zeyn/zeyn.css").await;
    assert!(
        shell.contains("@media (max-width: 1023px) and (scripting: enabled)"),
        "on phones and tablets the app menu starts folded, so the page does not jump when the script folds it"
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
async fn rails_render_their_first_page_on_the_server_with_canonical_links() {
    let router = app(None);
    let (status, page) = send(&router, get("/picks")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(r#"<a class="z-poster-link" href="/goods/1" data-zeyn-base-href>"#),
        "a path route is linked directly, not through /_zeyn/route: {page}"
    );
    assert!(!page.contains("/_zeyn/route/good_page"));
    assert!(page.contains(r#"data-zeyn-count="2" data-zeyn-size="2""#));
    let again = send(&router, get("/picks")).await.1;
    let main =
        |html: &str| html[html.find("<main").unwrap()..html.find("</main>").unwrap()].to_string();
    assert!(
        main(&page) == main(&again),
        "the cards are the same HTML for every visitor, so a public page can be cached"
    );
    let (status, more) = send(
        &router,
        post(
            "/_zeyn/grid",
            "shop.read",
            serde_json::json!({ "screen": "picks", "params": null, "node": "0", "query": { "page": { "type": "after", "cursor": "2", "limit": 99 } } }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{more}");
    assert!(
        more.contains(r#"href=\"/goods/3\""#),
        "later pages from the client use the same links: {more}"
    );
    let (status, opened) = send(&router, get("/goods/3")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(opened.contains("Good 3"));
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

#[screen(route = "/store", public)]
pub fn store() -> Screen {
    Screen::new("Store")
}

fn header_app(icon: bool) -> axum::Router {
    let place = if icon {
        CenterOnPhone::Icon
    } else {
        CenterOnPhone::Row
    };
    App::new("Test")
        .auth(|h: &HeaderMap| {
            let perms = h.get("x-perms")?.to_str().ok()?;
            Some(Identity::user("u").with_permissions(perms.split(',').map(String::from)))
        })
        .feature(
            Feature::new("store")
                .screen(store)
                .screen(results)
                .screen(good_page)
                .shell(Shell::site().header(|h| {
                    h.start(Paragraph::new("Hi <there>"))
                        .center(SearchBox::of(search_goods).results(results))
                        .center(Paragraph::new("Second"))
                        .center_on_phone(place)
                        .end(LinkButton::new("Good", good_page, GoodParams { id: 3 }))
                        .bottom(Chips::links("Topics").link(
                            "Results",
                            results,
                            SearchParams::default(),
                        ))
                        .theme()
                })),
        )
        .router()
        .unwrap()
}

#[tokio::test]
async fn header_slots_take_any_component_and_a_search_box_suggests_from_there() {
    let router = header_app(false);
    let (status, page) = send(&router, get("/store")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(
        page.contains(
            r#"data-zeyn-screen="_site.store" data-zeyn-params="null" data-center-phone="row""#
        ),
        "{page}"
    );
    let at = |s: &str| {
        page.find(s)
            .unwrap_or_else(|| panic!("{s} missing: {page}"))
    };
    assert!(at(r#"class="z-site-slot z-site-slot-start""#) < at(r#"class="z-site-brand""#));
    assert!(at(r#"class="z-site-brand""#) < at(r#"<nav class="z-site-nav""#));
    assert!(at(r#"<nav class="z-site-nav""#) < at(r#"id="z-site-slot-center""#));
    assert!(at(r#"id="z-site-slot-center""#) < at(r#"id="z-site-slot-end""#));
    assert!(at(r#"id="z-site-slot-end""#) < at(r#"id="z-site-actions""#));
    assert!(at(r#"id="z-site-actions""#) < at(r#"id="z-site-slot-bottom""#));
    assert!(page.contains("Hi &lt;there&gt;"));
    assert!(page.contains(r#"data-zeyn-node="1.0"><form class="z-search""#));
    assert!(page.contains(r#"data-zeyn-node="1.1""#));
    assert!(
        page.contains(r#"href="/goods/3""#),
        "links in slots use path routes"
    );
    assert!(page.contains(r#"aria-controls="z-site-nav z-site-actions""#));
    assert!(
        !page.contains("data-zeyn-site-center-toggle"),
        "a row needs no button"
    );

    let ask = |screen: &str, node: &str, perms: &str| {
        post(
            "/_zeyn/data",
            perms,
            serde_json::json!({ "screen": screen, "node": node, "key": "suggest", "input": "coffee" }),
        )
    };
    let (status, body) = send(&router, ask("_site.store", "1.0", "shop.read")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&body)
            .unwrap()
            .len(),
        6
    );
    let (status, _) = send(&router, ask("_site.store", "1.0", "")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the action's permission counts"
    );
    for (screen, node) in [
        ("_site.store", "1.1"),
        ("_site.store", "4.0"),
        ("_site.other", "1.0"),
    ] {
        let (status, _) = send(&router, ask(screen, node, "shop.read")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{screen} {node}");
    }

    let (_, page) = send(&header_app(true), get("/store")).await;
    assert!(page.contains(r#"data-center-phone="icon""#));
    assert!(page.contains(r#"<button type="button" class="z-site-toggle z-site-center-toggle" aria-expanded="false" aria-controls="z-site-slot-center" data-zeyn-site-center-toggle hidden>"#), "{page}");
    assert!(page.contains(r#"<span class="z-sr-only">Search</span>"#));
}

#[screen(route = "/finder", public)]
pub fn finder() -> Screen {
    Screen::new("Finder").body(
        SearchBox::of(search_goods)
            .results(results)
            .item(|g: &Good| {
                zeyn::ui::ListItem::new(g.name.clone())
                    .subtitle(g.city.clone())
                    .image(if g.id.0 == 5 {
                        "javascript:alert(1)".to_string()
                    } else if g.id.0 == 10 {
                        "https://elsewhere.example/x.png".to_string()
                    } else {
                        format!("/img/{}.png", g.id.0)
                    })
                    .link(good_page, GoodParams { id: g.id.0 })
            }),
    )
}

#[tokio::test]
async fn rich_suggestions_carry_text_subtitle_an_allowed_image_and_the_canonical_link() {
    let router = App::new("Test")
        .auth(|h: &HeaderMap| {
            let perms = h.get("x-perms")?.to_str().ok()?;
            Some(Identity::user("u").with_permissions(perms.split(',').map(String::from)))
        })
        .feature(
            Feature::new("shop")
                .screen(finder)
                .screen(results)
                .screen(good_page),
        )
        .router()
        .unwrap();
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/data",
            "shop.read",
            serde_json::json!({ "screen": "finder", "node": "0", "key": "suggest", "input": "coffee" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hits: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(
        hits[0],
        serde_json::json!({ "text": "Coffee 5", "subtitle": "Bursa", "image": null, "href": "/goods/5" }),
        "an unsafe image is left out"
    );
    assert_eq!(
        hits[1]["image"],
        serde_json::Value::Null,
        "a host outside img-src is left out"
    );
    assert_eq!(hits[2]["image"], "/img/15.png");
    assert_eq!(hits[2]["href"], "/goods/15");
}

fn app_header_app() -> axum::Router {
    App::new("Test")
        .auth(|h: &HeaderMap| {
            let perms = h.get("x-perms")?.to_str().ok()?;
            Some(Identity::user("u").with_permissions(perms.split(',').map(String::from)))
        })
        .header(|h| {
            h.center(SearchBox::of(search_goods).results(results))
                .end(LinkButton::new("Good <3>", good_page, GoodParams { id: 3 }))
                .end(Stack::new().add(Paragraph::new("Nested")))
        })
        .feature(
            Feature::new("store")
                .screen(store)
                .screen(results)
                .screen(good_page),
        )
        .router()
        .unwrap()
}

#[tokio::test]
async fn the_app_header_takes_the_site_header_slots_and_checks_data_permissions() {
    let router = app_header_app();
    let (status, page) = send(&router, get("/store")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"<div class="z-app" data-zeyn-shell="app">"#));
    let at = |s: &str| {
        page.find(s)
            .unwrap_or_else(|| panic!("{s} missing: {page}"))
    };
    assert!(
        at("</nav>")
            < at(
                r#"<header class="z-app-header" data-zeyn-screen="_app" data-zeyn-params="null">"#
            )
    );
    assert!(at(r#"id="z-app-header-slot-center""#) < at(r#"id="z-app-header-slot-end""#));
    assert!(at(r#"id="z-app-header-slot-end""#) < at(r#"<main class="z-main" id="z-main">"#));
    assert!(page.contains(r#"data-zeyn-node="1.0"><form class="z-search""#));
    assert!(page.contains("Good &lt;3&gt;"));

    let ask = |node: &str, perms: &str| {
        post(
            "/_zeyn/data",
            perms,
            serde_json::json!({ "screen": "_app", "node": node, "key": "suggest", "input": "coffee" }),
        )
    };
    let (status, body) = send(&router, ask("1.0", "shop.read")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = send(&router, ask("1.0", "")).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the action's permission counts"
    );
    for node in ["1.1", "2.1.0", "4.0", "2.0.0"] {
        let (status, _) = send(&router, ask(node, "shop.read")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{node}");
    }

    let plain = App::new("Test")
        .feature(Feature::new("store").screen(store))
        .router()
        .unwrap();
    let (_, page) = send(&plain, get("/store")).await;
    assert!(!page.contains("z-app-header"), "no header unless asked for");
    let (status, _) = send(&plain, ask("1.0", "shop.read")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[entity(table = "steps")]
pub struct Step {
    #[key]
    pub id: StepId,
    pub title: String,
    pub list: i64,
    pub rank: Rank,
}

/// Steps in two lists, ranked `F`, `V`, `k` in list 1 and `V` in list 2.
#[derive(Clone)]
pub struct Steps(std::sync::Arc<std::sync::Mutex<Vec<Step>>>);

impl Service for Steps {}

impl Steps {
    fn seeded() -> Steps {
        let step = |id: i64, title: &str, list: i64, rank: &str| Step {
            id: StepId(id),
            title: title.into(),
            list,
            rank: Rank::parse(rank).unwrap(),
        };
        Steps(std::sync::Arc::new(std::sync::Mutex::new(vec![
            step(1, "Plan <it>", 1, "F"),
            step(2, "Build", 1, "V"),
            step(3, "Ship", 1, "k"),
            step(4, "Other list", 2, "V"),
        ])))
    }

    /// The ids of list 1 in rank order.
    fn order(&self) -> Vec<i64> {
        let mut rows: Vec<(Rank, i64)> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.list == 1)
            .map(|s| (s.rank.clone(), s.id.0))
            .collect();
        rows.sort();
        rows.into_iter()
            .map(|(_, id)| id)
            .collect()
    }
}

#[action(permission = "steps.read")]
pub async fn search_steps(q: DataQuery, steps: Steps) -> Result<Page<Step>> {
    let mut rows: Vec<Step> = steps
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.list == 1)
        .cloned()
        .collect();
    rows.reverse();
    Ok(q.apply(rows))
}

#[action(permission = "steps.write")]
pub async fn reorder_steps(r: ReorderRequest<StepId>, steps: Steps) -> Result<()> {
    let mut all = steps.0.lock().unwrap();
    let rank = r.rank(|k| {
        all.iter()
            .find(|s| s.id == *k && s.list == 1)
            .map(|s| s.rank.clone())
    })?;
    let row = all
        .iter_mut()
        .find(|s| s.id == r.key && s.list == 1)
        .ok_or(Error::NotFound)?;
    row.rank = rank;
    Ok(())
}

#[screen(route = "/steps", permission = "steps.read")]
pub fn steps_screen() -> Screen {
    Screen::new("Steps")
        .body(Paragraph::new("Plan the release"))
        .body(
            Card::new()
                .add(Paragraph::new("Order"))
                .add(
                    Card::new().add(
                        List::of(search_steps)
                            .title("Steps")
                            .item(|s: &Step| zeyn::ui::ListItem::new(s.title.clone()))
                            .sort_option("By title", Step::title)
                            .sort_option("By id", Step::id)
                            .sortable(Step::rank, reorder_steps),
                    ),
                ),
        )
}

#[screen(route = "/steps/virtual", permission = "steps.read")]
pub fn steps_virtual() -> Screen {
    Screen::new("Steps").body(
        Card::new().add(
            List::of(search_steps)
                .item(|s: &Step| zeyn::ui::ListItem::new(s.title.clone()))
                .paging(Paging::virtual_scroll())
                .sortable(Step::rank, reorder_steps),
        ),
    )
}

fn steps_app(steps: Steps) -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(steps)
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
            Feature::new("steps")
                .screen(steps_screen)
                .screen(steps_virtual),
        )
        .router()
        .unwrap()
}

const STEPS_RW: &str = "steps.read,steps.write";

fn reorder(perms: &str, node: &str, input: serde_json::Value) -> Request<Body> {
    post(
        "/_zeyn/data",
        perms,
        serde_json::json!({ "screen": "steps_screen", "params": null, "node": node, "key": "reorder", "input": input }),
    )
}

#[tokio::test]
async fn sortable_lists_render_a_handle_per_row_and_load_their_script() {
    let router = steps_app(Steps::seeded());
    let req = Request::get("/steps")
        .header("x-perms", STEPS_RW)
        .body(Body::empty())
        .unwrap();
    let (status, page) = send(&router, req).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("data-zeyn-sortable"), "{page}");
    assert!(page.contains("/_zeyn/sortable.css?v="), "its stylesheet");
    assert!(page.contains(r#"data-zeyn-node="1.1.0""#), "a nested node");
    assert_eq!(
        page.matches(r#"class="z-sort-handle z-drag-handle""#)
            .count(),
        3
    );
    for key in ["1", "2", "3"] {
        assert!(page.contains(&format!(r#"data-zeyn-key="{key}""#)), "{key}");
    }
    assert!(page.contains("Plan &lt;it&gt;") && !page.contains("Plan <it>"));
    assert!(
        !page.contains("data-zeyn-cards-sort"),
        "the user's order is the order: no sort picker"
    );
    assert!(page.contains("Space lifts the row"));
    for name in ["sortable.js", "drag.js", "net.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    let (_, plain) = send(&app(None), get("/rows")).await;
    assert!(
        !plain.contains("/_zeyn/sortable.js"),
        "only with a sortable list"
    );
}

#[tokio::test]
async fn virtual_sortable_lists_stay_virtual_and_reorder_by_neighbor_keys() {
    let steps = Steps::seeded();
    let router = steps_app(steps.clone());
    let req = Request::get("/steps/virtual")
        .header("x-perms", STEPS_RW)
        .body(Body::empty())
        .unwrap();
    let (status, page) = send(&router, req).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(
        page.contains("data-zeyn-vlist") && page.contains("data-zeyn-sortable"),
        "a virtual list, not load more: {page}"
    );
    assert!(
        page.contains(r#"aria-posinset="1""#),
        "rows know their index"
    );
    assert_eq!(
        page.matches(r#"class="z-sort-handle z-drag-handle""#)
            .count(),
        3
    );
    assert!(page.contains("Space lifts the row"));
    for name in ["sortable.js", "vlist.js", "drag.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    assert!(page.contains(r#"data-zeyn-node="0.0""#), "a nested node");
    let input = serde_json::json!({ "key": "3", "after": null, "before": "1" });
    let req = post(
        "/_zeyn/data",
        STEPS_RW,
        serde_json::json!({ "screen": "steps_virtual", "params": null, "node": "0.0", "key": "reorder", "input": input }),
    );
    let (status, body) = send(&router, req).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(steps.order(), vec![3, 1, 2]);
}

#[tokio::test]
async fn reordering_takes_neighbor_keys_and_checks_csrf_and_permission() {
    let steps = Steps::seeded();
    let router = steps_app(steps.clone());
    let to_top = serde_json::json!({ "key": "3", "after": null, "before": "1" });
    let mut missing = reorder(STEPS_RW, "1.1.0", to_top.clone());
    missing
        .headers_mut()
        .remove("x-zeyn-csrf");
    assert_eq!(send(&router, missing).await.0, StatusCode::FORBIDDEN);
    let (status, _) = send(&router, reorder("steps.read", "1.1.0", to_top.clone())).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the reorder action's permission"
    );
    assert_eq!(steps.order(), [1, 2, 3]);
    let (status, body) = send(&router, reorder(STEPS_RW, "1.1.0", to_top.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(steps.order(), [3, 1, 2]);
    let page = Request::get("/steps")
        .header("x-perms", STEPS_RW)
        .body(Body::empty())
        .unwrap();
    let (_, page) = send(&router, page).await;
    let at = |text: &str| page.find(text).unwrap();
    assert!(
        at(">Ship<") < at(">Plan &lt;it&gt;<") && at(">Plan &lt;it&gt;<") < at(">Build<"),
        "the list asks the action for its rows by rank"
    );
    let (status, _) = send(&router, reorder(STEPS_RW, "1.1.0", to_top)).await;
    assert_eq!(status, StatusCode::OK, "the same request again");
    assert_eq!(steps.order(), [3, 1, 2], "gives the same order");
    let (status, _) = send(
        &router,
        reorder(
            STEPS_RW,
            "1.1.0",
            serde_json::json!({ "key": "1", "after": "2", "before": null }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(steps.order(), [3, 2, 1]);
    let bad = reorder(
        STEPS_RW,
        "x",
        serde_json::json!({ "key": "1", "after": null, "before": "3" }),
    );
    assert_eq!(
        send(&router, bad).await.0,
        StatusCode::UNPROCESSABLE_ENTITY,
        "not a node path"
    );
    for node in ["1.1", "1", "1.1.0.0"] {
        let (status, _) = send(
            &router,
            reorder(
                STEPS_RW,
                node,
                serde_json::json!({ "key": "1", "after": null, "before": "3" }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{node}");
    }
}

#[tokio::test]
async fn reordering_refuses_foreign_and_bad_neighbors_and_stale_orders() {
    let steps = Steps::seeded();
    let router = steps_app(steps.clone());
    let call = |input: serde_json::Value| reorder(STEPS_RW, "1.1.0", input);
    let cases = [
        (
            serde_json::json!({ "key": "1", "after": "4", "before": null }),
            StatusCode::NOT_FOUND,
            "a neighbor from another list",
        ),
        (
            serde_json::json!({ "key": "4", "after": "1", "before": "2" }),
            StatusCode::NOT_FOUND,
            "a row from another list",
        ),
        (
            serde_json::json!({ "key": "1", "after": "1", "before": null }),
            StatusCode::NOT_FOUND,
            "the row as its own neighbor",
        ),
        (
            serde_json::json!({ "key": "1", "after": "2", "before": "2" }),
            StatusCode::NOT_FOUND,
            "the same neighbor twice",
        ),
        (
            serde_json::json!({ "key": "x".repeat(201), "after": null, "before": "2" }),
            StatusCode::NOT_FOUND,
            "a key over the cap",
        ),
        (
            serde_json::json!({ "key": "abc", "after": null, "before": "2" }),
            StatusCode::NOT_FOUND,
            "a key of the wrong type",
        ),
        (
            serde_json::json!({ "key": 1 }),
            StatusCode::NOT_FOUND,
            "not the shape of a request",
        ),
        (
            serde_json::json!({ "key": "1", "after": "3", "before": "2" }),
            StatusCode::CONFLICT,
            "neighbors no longer in this order: someone moved them first",
        ),
    ];
    for (input, want, why) in cases {
        let (status, body) = send(&router, call(input)).await;
        assert_eq!(status, want, "{why}: {body}");
    }
    assert_eq!(steps.order(), [1, 2, 3], "nothing moved");
}
