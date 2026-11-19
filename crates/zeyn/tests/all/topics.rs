//! Topics: nodes that listen, controls that drive them, the refresh effect and the batched
//! refresh endpoint with its access checks; dependent lookups and the check of their pairs.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value as Json, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{LookupItem, LookupQuery};
use zeyn::{I18n, Identity};

#[entity(table = "sales")]
pub struct Sale {
    #[key]
    pub id: SaleId,
    pub region: String,
    pub amount: i64,
}

/// The sales rows and the number of times the grid's action ran.
#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Sale>>>, Arc<AtomicUsize>);

impl Service for Store {}

#[action(permission = "sales.read")]
pub async fn search_sales(q: DataQuery, store: Store) -> Result<Page<Sale>> {
    store.1.fetch_add(1, Ordering::SeqCst);
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "sales.write")]
pub async fn add_sale(sale: Valid<Sale>, store: Store) -> Result<Sale> {
    store
        .0
        .write()
        .unwrap()
        .push(sale.clone().into_inner());
    Ok(sale.into_inner())
}

#[params]
pub struct SalesParams {
    pub region: Option<String>,
}

#[model]
pub struct SalesFilter {
    pub region: Option<String>,
    pub limit: Option<i64>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Store) -> Screen {
    let total: i64 = store
        .0
        .read()
        .unwrap()
        .iter()
        .filter(|s| {
            p.region
                .as_deref()
                .is_none_or(|r| s.region == r)
        })
        .map(|s| s.amount)
        .sum();
    let mut grid = DataGrid::of(search_sales);
    if let Some(region) = p.region.clone() {
        grid = grid.scope(Sale::region, region);
    }
    let grid = grid.column(Sale::amount);
    Screen::new("Sales")
        .body(
            Chips::filter("Region", sales, |v| SalesParams {
                region: v.first().map(|s| s.to_string()),
            })
            .current(p.region.clone())
            .option("north", "North")
            .option("south", "South")
            .drives("sales"),
        )
        .body(
            Row::new()
                .col(Kpi::new("Total", total as f64).listens("sales"))
                .col(grid.listens("sales")),
        )
        .body(
            Form::of(SalesFilter {
                region: p.region.clone(),
                limit: None,
            })
            .field(SalesFilter::region)
            .field(SalesFilter::limit)
            .drives("sales"),
        )
        .body(
            Form::of(Sale::default())
                .field(Sale::region)
                .field(Sale::amount)
                .submit("Add", add_sale)
                .on_success(|_, ui| ui.refresh("sales")),
        )
}

#[model]
pub struct Address {
    pub country: Option<i64>,
    pub city: Option<i64>,
}

const CITIES: [(i64, i64, &str); 4] = [
    (11, 1, "Istanbul"),
    (12, 1, "Izmir"),
    (21, 2, "Berlin"),
    (22, 2, "Hamburg"),
];

#[action(permission = "geo.read")]
pub async fn find_countries(q: LookupQuery) -> Result<Page<LookupItem>> {
    Ok(q.apply(vec![
        LookupItem::new(1, "Turkey"),
        LookupItem::new(2, "Germany"),
    ]))
}

#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery) -> Result<Page<LookupItem>> {
    let country = q.parent_id();
    let items = CITIES
        .iter()
        .filter(|(_, c, _)| Some(*c) == country)
        .map(|(id, _, name)| LookupItem::new(*id, *name))
        .collect();
    Ok(q.apply(items))
}

#[action(permission = "addr.write")]
pub async fn save_address(a: Valid<Address>) -> Result<Address> {
    Ok(a.into_inner())
}

#[screen(route = "/address", permission = "addr.write")]
pub fn address() -> Screen {
    Screen::new("Address").body(
        Form::of(Address::default())
            .lookup(Address::country, find_countries)
            .lookup(Address::city, find_cities)
            .depends_on(Address::city, Address::country)
            .submit("Save", save_address),
    )
}

fn app() -> Router {
    app_counting(Arc::default())
}

/// The test app, counting the runs of the grid's action in `calls`.
fn app_counting(calls: Arc<AtomicUsize>) -> Router {
    let rows = [("north", 10), ("north", 5), ("south", 7)]
        .into_iter()
        .zip(1..)
        .map(|((region, amount), id)| Sale {
            id: SaleId(id),
            region: region.into(),
            amount,
        })
        .collect();
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(Store(Arc::new(RwLock::new(rows)), calls))
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
            Feature::new("sales")
                .screen(sales)
                .screen(address),
        )
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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

fn get(path: &str, perms: &str) -> Request<Body> {
    Request::get(path)
        .header("x-perms", perms)
        .body(Body::empty())
        .unwrap()
}

fn post(path: &str, perms: &str, body: Json) -> Request<Body> {
    Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn the_page_marks_listeners_and_drivers_and_links_the_topics_module() {
    let (status, page) = send(&app(), get("/sales", "sales.read")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"<div class="z-listen" data-zeyn-listens="sales"><div class="z-node" data-zeyn-node="1.0.0">"#));
    assert!(page.contains(r#"role="group" aria-label="Region" data-zeyn-drives="sales""#));
    assert!(page.contains(r#"data-zeyn-params="{&quot;region&quot;:&quot;north&quot;}""#));
    assert!(page.contains(
        r#"data-zeyn-drives="sales" data-zeyn-kinds="{&quot;limit&quot;:&quot;number&quot;}""#
    ));
    assert!(page.contains("/_zeyn/topics.js?v="));
    let (_, other) = send(&app(), get("/address", "addr.write")).await;
    assert!(
        !other.contains("/_zeyn/topics.js"),
        "a page without topics never loads the module"
    );
}

#[tokio::test]
async fn nested_listeners_render_again_in_one_request_with_the_driven_params() {
    let router = app();
    let body =
        json!({ "screen": "sales", "params": { "region": "north" }, "nodes": ["1.0", "0", "1.0"] });
    let (status, text) = send(&router, post("/_zeyn/nodes", "sales.read", body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let nodes: Json = serde_json::from_str(&text).unwrap();
    let nodes = nodes["nodes"].as_object().unwrap();
    assert_eq!(nodes.len(), 2, "a path asked twice is rendered once");
    let kpi = nodes["1.0"].as_str().unwrap();
    assert!(kpi.starts_with(r#"<div class="z-node" data-zeyn-node="1.0"><div class="z-listen""#));
    assert!(
        kpi.contains("15"),
        "the listener reads the driven region from its params: {kpi}"
    );
    let chips = nodes["0"].as_str().unwrap();
    assert!(chips.contains(r#"aria-pressed="true""#), "{chips}");
    assert!(
        chips.contains(
            r#"data-value="north" data-zeyn-base-href data-zeyn-params="{&quot;region&quot;:null}""#
        ),
        "{chips}"
    );
}

#[tokio::test]
async fn overlapping_refresh_paths_load_a_grid_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = app_counting(calls.clone());
    let body = json!({
        "screen": "sales",
        "params": null,
        "nodes": ["1.1", "1", "1.1", "1", "1.0"]
    });
    let (status, text) = send(&router, post("/_zeyn/nodes", "sales.read", body)).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a node inside another requested node is rendered with its ancestor"
    );
    let nodes: Json = serde_json::from_str(&text).unwrap();
    let nodes = nodes["nodes"].as_object().unwrap();
    assert_eq!(nodes.len(), 1, "{text}");
    assert!(
        nodes["1"]
            .as_str()
            .unwrap()
            .contains(r#"data-zeyn-node="1.1"#)
    );
}

#[tokio::test]
async fn the_refresh_endpoint_checks_csrf_access_and_paths() {
    let router = app();
    let body = json!({ "screen": "sales", "params": null, "nodes": ["1.0"] });
    let (status, _) = send(&router, post("/_zeyn/nodes", "", body.clone())).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the screen's permission is checked"
    );
    let no_csrf = Request::post("/_zeyn/nodes")
        .header("x-perms", "sales.read")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, _) = send(&router, no_csrf).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "posts need the CSRF token");
    let missing = json!({ "screen": "sales", "nodes": ["1.0", "1.9.4"] });
    let (status, _) = send(&router, post("/_zeyn/nodes", "sales.read", missing)).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "every path must be in the screen"
    );
    let many: Vec<String> = (0..33)
        .map(|i| format!("1.{i}"))
        .collect();
    let (status, _) = send(
        &router,
        post(
            "/_zeyn/nodes",
            "sales.read",
            json!({ "screen": "sales", "nodes": many }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "at most 32 nodes per request"
    );
    let repeated: Vec<&str> = vec!["1.0"; 20_000];
    let (status, _) = send(
        &router,
        post(
            "/_zeyn/nodes",
            "sales.read",
            json!({ "screen": "sales", "nodes": repeated }),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a long list is refused before it is deduplicated, also when every path is the same"
    );
    let bad = json!({ "screen": "sales", "nodes": ["1..0"] });
    let (status, _) = send(&router, post("/_zeyn/nodes", "sales.read", bad)).await;
    assert!(status.is_client_error(), "node paths are checked: {status}");
    let wrong = json!({ "screen": "sales", "params": { "region": 5 }, "nodes": ["1.0"] });
    let (status, _) = send(&router, post("/_zeyn/nodes", "sales.read", wrong)).await;
    assert!(
        status.is_client_error(),
        "params are read as the screen's typed params: {status}"
    );
}

#[tokio::test]
async fn a_saved_form_asks_the_page_to_refresh_its_topic() {
    let form =
        json!({ "screen": "sales", "node": "3", "values": { "region": "south", "amount": "4" } });
    let (status, _) = send(&app(), post("/_zeyn/form", "sales.read", form.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "saving needs sales.write");
    let (status, body) = send(&app(), post("/_zeyn/form", "sales.read,sales.write", form)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(r#"{"effect":"refresh","topics":["sales"]}"#),
        "{body}"
    );
}

#[test]
fn the_page_check_reports_topics_nobody_listens_to_and_bad_settings() {
    let app = App::new("T").feature(Feature::new("t").screen(address));
    let ok = Screen::new("Ok")
        .body(Chips::filter("R", address, |_| ()).drives("sales"))
        .body(Kpi::new("A", 1.0).listens(["sales", "stock"]));
    assert!(app.check_page(&ok).is_ok());
    let lonely = Screen::new("Lonely").body(
        Row::new().col(
            Form::of(SalesFilter::default())
                .field(SalesFilter::region)
                .drives("sales"),
        ),
    );
    let problems = app.check_page(&lonely).unwrap_err();
    assert!(
        problems[0].contains("node 0.0 drives topic `sales`, but nothing on this page listens"),
        "{problems:?}"
    );
    let bad = Screen::new("Bad")
        .body(Kpi::new("A", 1.0).listens("a,b"))
        .body(
            Chips::links("L")
                .link("One", address, ())
                .drives("x"),
        )
        .body(Kpi::new("B", 1.0).listens("x"))
        .body(
            Form::of(Address::default())
                .field(Address::country)
                .depends_on(Address::city, Address::country)
                .depends_on(Address::country, Address::country),
        );
    let problems = app
        .check_page(&bad)
        .unwrap_err()
        .join("\n");
    assert!(
        problems.contains("topic `a,b` is not a valid name"),
        "{problems}"
    );
    assert!(
        problems.contains("works on filter chips only"),
        "{problems}"
    );
    assert!(
        problems.contains("`depends_on` names `city`, which is not a lookup"),
        "{problems}"
    );
    assert!(
        problems.contains("lookup `country` depends on `country`"),
        "{problems}"
    );
}

#[tokio::test]
async fn a_dependent_lookup_searches_with_its_parent_and_submits_only_matching_pairs() {
    let router = app();
    let (_, page) = send(&router, get("/address", "addr.write,geo.read")).await;
    assert!(
        page.contains(r#"data-zeyn-lookup="city" data-multiple="false" data-depends-on="country""#)
    );
    let search = json!({ "screen": "address", "node": "0", "key": "city", "input": { "text": "", "limit": 20, "parent": "2" } });
    let (status, body) = send(&router, post("/_zeyn/data", "addr.write,geo.read", search)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Berlin") && !body.contains("Izmir"), "{body}");

    let submit = |country: &str, city: &str| json!({ "screen": "address", "node": "0", "values": { "country": country, "city": city } });
    let (status, body) = send(
        &router,
        post("/_zeyn/form", "addr.write,geo.read", submit("1", "12")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send(
        &router,
        post("/_zeyn/form", "addr.write,geo.read", submit("1", "21")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a city of another country: {body}"
    );
    assert!(body.contains("\"city\""), "{body}");
    let (status, _) = send(
        &router,
        post("/_zeyn/form", "addr.write", submit("1", "12")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "without the lookup's permission the pair cannot be confirmed"
    );
    let (status, body) = send(&router, post("/_zeyn/form", "addr.write", submit("", ""))).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "empty fields are not checked: {body}"
    );
}
