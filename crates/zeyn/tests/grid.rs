//! Grid features end to end: totals, filters, sorting, views, row actions, selection, editing
//! and export, through the HTTP endpoints with permissions.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Aggregate, AggregateRequest, I18n, Identity, Value};

/// Item used by the grid tests.
#[entity(table = "items")]
pub struct Item {
    #[key]
    pub id: ItemId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub kind: Kind,
    #[rule(min = 0)]
    pub price: Money,
    pub qty: i64,
    pub active: bool,
    pub added: Date,
    pub due: Option<Date>,
    #[readonly]
    pub note: String,
    #[version]
    pub version: i64,
}

/// Item kind.
#[choice]
pub enum Kind {
    Tool,
    Part,
    Food,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Item>>>);

impl Service for Store {}

fn item(i: i64) -> Item {
    Item {
        id: ItemId(i),
        code: format!("C-{i:02}"),
        kind: match i % 3 {
            0 => Kind::Tool,
            1 => Kind::Part,
            _ => Kind::Food,
        },
        price: Money::tl(i * 1_000),
        qty: i,
        active: i % 2 == 0,
        added: Date::new(2026, 1, i as u8).unwrap(),
        due: (i % 4 == 0).then(|| Date::new(2026, 3, i as u8).unwrap()),
        note: format!("note {i}"),
        version: 1,
    }
}

impl Store {
    fn seeded() -> Self {
        Store(Arc::new(RwLock::new((1..=20).map(item).collect())))
    }
}

#[action(permission = "items.read")]
pub async fn search(q: GridQuery, store: Store) -> Result<Page<Item>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[screen(route = "/items", permission = "items.read")]
pub fn list() -> Screen {
    Screen::new("Items").body(
        Grid::of(search)
            .paging(Paging::pages(5))
            .column(Item::code)
            .filter()
            .count()
            .column(Item::kind)
            .filter()
            .column(Item::price)
            .filter()
            .sum()
            .avg()
            .column(Item::qty)
            .sum()
            .avg()
            .min()
            .max()
            .column(Item::added)
            .filter()
            .min()
            .max()
            .column(Item::active)
            .filter(),
    )
}

fn app(identity: Identity) -> App {
    App::new("Test")
        .i18n(I18n::new("en").catalog("tr", "{}").unwrap())
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("items").screen(list).api(search))
}

fn router(identity: Identity) -> Router {
    app(identity).router().unwrap()
}

fn reader() -> Identity {
    Identity::user("ayse").with_permissions(["items.read"])
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn send(router: &Router, req: Request<Body>) -> Reply {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    Reply { status, headers, body }
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn post_as(path: &str, body: serde_json::Value, locale: &str) -> Request<Body> {
    Request::post(path)
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}; zeyn_locale={locale}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post(path: &str, body: serde_json::Value) -> Request<Body> {
    post_as(path, body, "en")
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).header("x-user", "1").body(Body::empty()).unwrap()
}

fn agg(field: &str, op: Aggregate) -> AggregateRequest {
    AggregateRequest { field: field.into(), op }
}

fn query(json: serde_json::Value) -> GridQuery {
    serde_json::from_value(json).unwrap()
}

#[test]
fn totals_cover_every_filtered_row_not_only_the_page() {
    let rows: Vec<Item> = (1..=20).map(item).collect();
    let mut q = query(serde_json::json!({ "page": { "type": "offset", "offset": 0, "limit": 2 } }));
    q.aggregates = vec![
        agg("qty", Aggregate::Sum),
        agg("qty", Aggregate::Avg),
        agg("qty", Aggregate::Min),
        agg("qty", Aggregate::Max),
        agg("price", Aggregate::Sum),
        agg("price", Aggregate::Avg),
        agg("due", Aggregate::Count),
        agg("added", Aggregate::Max),
    ];
    let page = q.apply(rows);
    assert_eq!(page.items.len(), 2);
    let value = |field: &str, op: Aggregate| page.totals.iter().find(|t| t.field == field && t.op == op).unwrap().value.clone();
    assert_eq!(value("qty", Aggregate::Sum), Value::Int(210));
    assert_eq!(value("qty", Aggregate::Avg), Value::Decimal(10.5));
    assert_eq!(value("qty", Aggregate::Min), Value::Int(1));
    assert_eq!(value("qty", Aggregate::Max), Value::Int(20));
    assert_eq!(value("price", Aggregate::Sum), Value::Money(Money::tl(210_000)));
    assert_eq!(value("price", Aggregate::Avg), Value::Money(Money::tl(10_500)));
    assert_eq!(value("due", Aggregate::Count), Value::Int(5));
    assert_eq!(value("added", Aggregate::Max), Value::Date(Date::new(2026, 1, 20).unwrap()));
}

#[test]
fn totals_ignore_unknown_fields_and_ops_that_do_not_fit() {
    let mut q = GridQuery::default();
    q.aggregates = vec![agg("nope", Aggregate::Sum), agg("code", Aggregate::Sum), agg("code", Aggregate::Count)];
    let page = q.apply((1..=3).map(item).collect());
    assert_eq!(page.totals.len(), 1);
    assert_eq!(page.totals[0].value, Value::Int(3));
}

#[test]
fn money_average_rounds_half_away_from_zero() {
    let values = [Value::Money(Money::tl(1)), Value::Money(Money::tl(2))].into_iter();
    assert_eq!(zeyn::total(values, Aggregate::Avg), Some(Value::Money(Money::tl(2))));
    let values = [Value::Money(Money::tl(-1)), Value::Money(Money::tl(-2))].into_iter();
    assert_eq!(zeyn::total(values, Aggregate::Avg), Some(Value::Money(Money::tl(-2))));
    let mixed = [Value::Money(Money::tl(1)), Value::Money(Money::new(1, zeyn::Currency::Usd))].into_iter();
    assert_eq!(zeyn::total(mixed, Aggregate::Sum), None);
    assert_eq!(zeyn::total(std::iter::empty(), Aggregate::Avg), Some(Value::Null));
}

#[tokio::test]
async fn page_renders_footer_totals_with_the_first_page() {
    let r = send(&router(reader()), get("/items")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.contains(r#"<tfoot data-zeyn-foot>"#));
    assert!(r.body.contains(r#"data-field="qty" data-agg="sum avg min max""#));
    assert!(r.body.contains(r#"<span class="z-agg-label">Sum</span> <span class="z-agg-value">210</span>"#));
    assert!(r.body.contains("₺2,100.00"));
    assert!(r.body.contains(r#"<span class="z-agg-value">10.50</span>"#));
}

#[tokio::test]
async fn grid_endpoint_returns_totals_of_the_filtered_set_in_the_users_language() {
    let body = serde_json::json!({
        "screen": "list", "node": 0,
        "query": {
            "page": { "type": "offset", "offset": 0, "limit": 2 },
            "filters": [{ "field": "code", "contains": "c-1" }],
            "with_total": true,
            "aggregates": [{ "field": "price", "op": "sum" }, { "field": "qty", "op": "avg" }]
        }
    });
    let r = send(&router(reader()), post_as("/_zeyn/grid", body, "tr")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["total"], 10);
    let foot = j["foot"].as_str().unwrap();
    assert!(foot.contains("1.450,00 ₺"), "{foot}");
    assert!(foot.contains("14,50"), "{foot}");
    assert!(foot.contains("Toplam"), "{foot}");
}

#[tokio::test]
async fn grid_endpoint_computes_only_totals_the_grid_declares() {
    let body = serde_json::json!({
        "screen": "list", "node": 0,
        "query": { "aggregates": [{ "field": "note", "op": "count" }, { "field": "kind", "op": "count" }] }
    });
    let r = send(&router(reader()), post("/_zeyn/grid", body)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.json()["foot"].is_null());
    let r = send(&router(reader()), post("/_zeyn/grid", serde_json::json!({ "screen": "list", "node": 0 }))).await;
    assert!(r.json()["foot"].is_null());
}

#[tokio::test]
async fn grid_endpoint_needs_the_read_permission() {
    let r = send(&router(Identity::user("x")), post("/_zeyn/grid", serde_json::json!({ "screen": "list", "node": 0 }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let _ = r.headers;
}
