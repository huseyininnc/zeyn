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

/// Event with a moment, for time zone filters.
#[entity(table = "events")]
pub struct Event {
    #[key]
    pub id: EventId,
    pub at: DateTime,
}

fn codes(page: &Page<Item>) -> Vec<String> {
    page.items.iter().map(|i| i.code.clone()).collect()
}

fn filtered(filters: Vec<zeyn::Filter>) -> Page<Item> {
    let q = GridQuery { filters, page: zeyn::PageRequest::Offset { offset: 0, limit: 100 }, ..GridQuery::default() };
    q.apply((1..=20).map(item).collect())
}

#[test]
fn number_and_money_ranges_are_inclusive_and_open_ended() {
    use zeyn::Filter;
    assert_eq!(codes(&filtered(vec![Filter::range("qty", Some("18"), None)])), ["C-18", "C-19", "C-20"]);
    assert_eq!(codes(&filtered(vec![Filter::range("price", Some("10"), Some("30.00"))])), ["C-01", "C-02", "C-03"]);
    assert_eq!(filtered(vec![Filter::range("price", Some("10,5"), Some("20"))]).items.len(), 1);
    assert_eq!(filtered(vec![Filter::range("qty", Some("abc"), Some("1e9"))]).items.len(), 20);
}

#[test]
fn date_ranges_and_optional_dates() {
    use zeyn::Filter;
    let page = filtered(vec![Filter::range("added", Some("2026-01-05"), Some("2026-01-07"))]);
    assert_eq!(codes(&page), ["C-05", "C-06", "C-07"]);
    let page = filtered(vec![Filter::range("due", None, Some("2026-03-08"))]);
    assert_eq!(codes(&page), ["C-04", "C-08"]);
}

#[test]
fn moment_date_filters_use_the_users_whole_day() {
    use zeyn::Filter;
    let at = DateTime::parse("2026-03-01T22:30:00Z").unwrap();
    let rows = vec![Event { id: EventId(1), at }];
    let mut q = GridQuery { filters: vec![Filter::range("at", Some("2026-03-02"), Some("2026-03-02"))], ..GridQuery::default() };
    assert_eq!(q.apply(rows.clone()).items.len(), 0);
    q.time_zone = TimeZone::named("Europe/Istanbul").unwrap();
    assert_eq!(q.apply(rows.clone()).items.len(), 1);
    q.filters = vec![Filter::range("at", Some("2026-03-02T01:00"), Some("2026-03-02T01:30"))];
    assert_eq!(q.apply(rows).items.len(), 1);
}

#[test]
fn choices_booleans_and_text_operators() {
    use zeyn::Filter;
    assert_eq!(filtered(vec![Filter::any_of("kind", ["Tool", "nope"])]).items.len(), 6);
    assert_eq!(filtered(vec![Filter::any_of("kind", ["nope"])]).items.len(), 0);
    assert_eq!(filtered(vec![Filter::is("active", true)]).items.len(), 10);
    let starts = Filter { field: "code".into(), starts_with: Some("c-1".into()), ..Filter::default() };
    assert_eq!(filtered(vec![starts]).items.len(), 10);
    let equals = Filter { field: "code".into(), equals: Some("C-07".into()), ..Filter::default() };
    assert_eq!(codes(&filtered(vec![equals])), ["C-07"]);
    let too_long = Filter::contains("code", "x".repeat(zeyn::MAX_FILTER_TEXT + 1));
    assert_eq!(filtered(vec![too_long]).items.len(), 20);
}

#[test]
fn old_contains_filters_still_read() {
    let q: GridQuery = query(serde_json::json!({ "filters": [{ "field": "code", "contains": "C-2" }] }));
    assert_eq!(codes(&q.apply((1..=20).map(item).collect())), ["C-20"]);
}

fn grid_call(query: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": "list", "node": 0, "query": query })
}

#[tokio::test]
async fn grid_endpoint_reads_typed_filters() {
    let q = serde_json::json!({
        "with_total": true,
        "filters": [
            { "field": "kind", "any_of": ["Part", "Food"] },
            { "field": "price", "from": "50", "to": "150" },
            { "field": "active", "is": true }
        ]
    });
    let r = send(&router(reader()), post("/_zeyn/grid", grid_call(q))).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["total"], 3, "{}", r.body);
}

#[tokio::test]
async fn grid_endpoint_ignores_filters_and_sorts_on_fields_it_does_not_offer() {
    let q = serde_json::json!({
        "with_total": true,
        "filters": [{ "field": "note", "contains": "note 1" }, { "field": "qty", "from": "1" }, { "field": "secret", "contains": "x" }],
        "sort": [{ "field": "note", "desc": true }]
    });
    let r = send(&router(reader()), post("/_zeyn/grid", grid_call(q))).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["total"], 20);
    let rows = j["rows"].as_str().unwrap();
    assert!(rows.find("C-01").unwrap() < rows.find("C-02").unwrap());
}

#[tokio::test]
async fn grid_endpoint_sorts_by_several_columns() {
    let q = serde_json::json!({
        "page": { "type": "offset", "offset": 0, "limit": 4 },
        "sort": [{ "field": "kind", "desc": false }, { "field": "qty", "desc": true }]
    });
    let r = send(&router(reader()), post("/_zeyn/grid", grid_call(q))).await;
    let rows = r.json()["rows"].as_str().unwrap().to_string();
    let order: Vec<usize> = ["C-20", "C-17", "C-14", "C-11"].iter().map(|c| rows.find(c).unwrap()).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{rows}");
}

#[tokio::test]
async fn header_names_filter_kinds_and_localized_choices() {
    let req = Request::get("/items").header("x-user", "1").header(header::COOKIE, "zeyn_locale=tr").body(Body::empty()).unwrap();
    let r = send(&router(reader()), req).await;
    assert!(r.body.contains(r#"data-field="price" aria-sort="none" aria-colindex="3" data-filter="money""#), "{}", r.body);
    assert!(r.body.contains(r#"data-filter="choice" data-options=""#));
    assert!(r.body.contains(r#"data-filter="bool""#));
    assert!(r.body.contains(r#"data-filter="date""#));
    assert!(r.body.contains("data-zeyn-texts="));
    assert!(r.body.contains("Tümü"));
}

fn views_router(store: zeyn::MemoryViewStore) -> Router {
    App::new("Test")
        .service(Store::seeded())
        .datagrid_views(store)
        .auth(|h: &HeaderMap| {
            let user = h.get("x-user")?.to_str().ok()?.to_string();
            let perms: Vec<&str> = if user == "guest" { vec![] } else { vec!["items.read"] };
            Some(Identity::user(user).with_permissions(perms))
        })
        .feature(Feature::new("items").screen(list))
        .router()
        .unwrap()
}

fn as_user(mut req: Request<Body>, user: &str) -> Request<Body> {
    req.headers_mut().insert("x-user", user.parse().unwrap());
    req
}

fn save_view(view: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": "list", "node": 0, "view": view })
}

#[tokio::test]
async fn saved_view_comes_back_on_the_next_page_load_with_its_sort_and_filters() {
    let store = zeyn::MemoryViewStore::default();
    let router = views_router(store.clone());
    let view = serde_json::json!({
        "columns": [{ "field": "qty", "width": 5000 }, { "field": "code", "pin": "start" }, { "field": "secret" }, { "field": "qty" }],
        "sort": [{ "field": "qty", "desc": true }, { "field": "note", "desc": false }],
        "filters": [{ "field": "kind", "any_of": ["Tool"] }, { "field": "note", "contains": "x" }]
    });
    let r = send(&router, as_user(post("/_zeyn/grid/view", save_view(view)), "ayse")).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.body);

    let page = send(&router, as_user(get("/items"), "ayse")).await;
    assert!(page.body.contains(r#"data-zeyn-views="server""#));
    let head = page.body.split("<thead>").nth(1).unwrap().split("</thead>").next().unwrap();
    assert!(head.find(r#"data-field="code""#).unwrap() < head.find(r#"data-field="qty""#).unwrap());
    assert!(head.contains(r#"data-pin="start""#));
    assert!(head.contains("width:1200px;"), "{head}");
    assert!(!head.contains(r#"data-field="kind""#));
    assert!(page.body.contains(r#"data-zeyn-total="6""#), "only tools, the note filter is not offered");
    let rows = page.body.split("<tbody data-zeyn-rows>").nth(1).unwrap();
    assert!(rows.starts_with("<tr data-zeyn-key=\"18\"><td>C-18</td><td class=\"z-num\">18</td></tr>"), "{}", &rows[..120]);

    let other = send(&router, as_user(get("/items"), "mert")).await;
    assert!(!other.body.contains("data-zeyn-view="));
    assert!(other.body.contains(r#"data-zeyn-total="20""#));

    let r = send(&router, as_user(post("/_zeyn/grid/view", save_view(serde_json::Value::Null)), "ayse")).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let page = send(&router, as_user(get("/items"), "ayse")).await;
    assert!(!page.body.contains("data-zeyn-view="));
}

#[tokio::test]
async fn saving_a_view_needs_a_store_a_user_and_read_access() {
    let views = views_router(zeyn::MemoryViewStore::default());
    let r = send(&views, as_user(post("/_zeyn/grid/view", save_view(serde_json::json!({}))), "guest")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = send(&router(reader()), post("/_zeyn/grid/view", save_view(serde_json::json!({})))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&views, as_user(post("/_zeyn/grid/view", serde_json::json!({ "screen": "list", "node": 7, "view": {} })), "ayse")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn grid_endpoint_renders_rows_and_header_in_the_requested_layout() {
    let body = serde_json::json!({
        "screen": "list", "node": 0, "head": true,
        "query": { "page": { "type": "offset", "offset": 0, "limit": 1 }, "aggregates": [{ "field": "qty", "op": "sum" }, { "field": "price", "op": "sum" }] },
        "view": { "columns": [{ "field": "price", "pin": "end" }, { "field": "qty" }, { "field": "note" }] }
    });
    let r = send(&router(reader()), post("/_zeyn/grid", body)).await;
    let j = r.json();
    assert_eq!(j["rows"], "<tr data-zeyn-key=\"1\"><td class=\"z-num\">1</td><td class=\"z-num\">₺10.00</td></tr>");
    let head = j["head"].as_str().unwrap();
    assert!(head.find("qty").unwrap() < head.find("price").unwrap());
    assert!(head.contains(r#"data-pin="end""#));
    let foot = j["foot"].as_str().unwrap();
    assert!(foot.find(">210<").unwrap() < foot.find("₺2,100.00").unwrap(), "{foot}");
}

#[test]
fn at_most_three_columns_stay_pinned_per_side() {
    use zeyn::{DataGridView, GridSource};
    let grid = Grid::of(search).column(Item::code).column(Item::kind).column(Item::price).column(Item::qty).column(Item::added).done();
    let columns: Vec<serde_json::Value> =
        ["code", "kind", "price", "qty", "added"].iter().map(|f| serde_json::json!({ "field": f, "pin": "start" })).collect();
    let view: DataGridView = serde_json::from_value(serde_json::json!({ "columns": columns })).unwrap();
    let clean = GridSource::clean_view(&grid, view);
    assert_eq!(clean.columns.iter().filter(|c| c.pin.is_some()).count(), 3);
    assert_eq!(clean.columns.len(), 5);
}

#[action(permission = "items.write")]
pub async fn bump(id: ItemId, store: Store) -> Result<()> {
    let mut rows = store.0.write().unwrap();
    let row = rows.iter_mut().find(|r| r.id == id).ok_or(Error::NotFound)?;
    if row.qty > 100 {
        return Err(Error::Denied("too many".into()));
    }
    row.qty += 100;
    Ok(())
}

#[screen(route = "/act", permission = "items.read")]
pub fn act() -> Screen {
    Screen::new("Act").body(
        Grid::of(search)
            .column(Item::code)
            .column(Item::kind)
            .badge(|k| match k {
                Kind::Tool => zeyn::Tone::Info,
                _ => zeyn::Tone::Neutral,
            })
            .column(Item::qty)
            .progress(40)
            .column(Item::active)
            .check_icon()
            .row_action("Bump", bump, |i| i.id),
    )
}

fn act_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("items").screen(act))
        .router()
        .unwrap();
    (router, store)
}

fn run(key: &str, item: &str) -> Request<Body> {
    post("/_zeyn/grid/run", serde_json::json!({ "screen": "act", "node": 0, "key": key, "item": item }))
}

#[tokio::test]
async fn cells_render_badges_progress_and_icons() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/act")).await;
    assert!(r.body.contains(r#"<span class="z-badge z-tone-neutral">Part</span>"#));
    assert!(r.body.contains(r#"<span class="z-badge z-tone-info">Tool</span>"#));
    assert!(r.body.contains(r#"style="--z-pct:2.5%""#), "1 of 40");
    assert!(r.body.contains(r#"<span class="z-bool z-bool-no" role="img" aria-label="No"></span>"#));
    assert!(r.body.contains(r#"<tr data-zeyn-key="1">"#));
}

#[tokio::test]
async fn row_action_buttons_show_only_with_the_actions_permission() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/act")).await;
    assert!(r.body.contains(r#"<td class="z-row-actions"></td>"#));
    assert!(!r.body.contains("data-zeyn-run="));
    let writer = Identity::user("w").with_permissions(["items.read", "items.write"]);
    let (router, _) = act_router(writer);
    let r = send(&router, get("/act")).await;
    assert!(r.body.contains(r#"data-zeyn-run="a0""#));
}

#[tokio::test]
async fn row_action_runs_with_input_built_from_the_row_on_the_server() {
    let writer = Identity::user("w").with_permissions(["items.read", "items.write"]);
    let (router, store) = act_router(writer);
    let r = send(&router, run("7", "a0")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(store.0.read().unwrap()[6].qty, 107);
    let r = send(&router, run("7", "a0")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "the action's own error comes back");
}

#[tokio::test]
async fn row_action_is_refused_without_permission_unknown_items_or_rows() {
    let (router, store) = act_router(reader());
    assert_eq!(send(&router, run("7", "a0")).await.status, StatusCode::FORBIDDEN);
    assert_eq!(store.0.read().unwrap()[6].qty, 7);
    let writer = Identity::user("w").with_permissions(["items.read", "items.write"]);
    let (router, _) = act_router(writer.clone());
    assert_eq!(send(&router, run("7", "a1")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, run("7", "x")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, run("999", "a0")).await.status, StatusCode::NOT_FOUND);
    let (router, _) = act_router(Identity::user("w").with_permissions(["items.write"]));
    assert_eq!(send(&router, run("7", "a0")).await.status, StatusCode::FORBIDDEN, "screen access first");
}

#[action(permission = "items.write")]
pub async fn deactivate(ids: Vec<ItemId>, store: Store) -> Result<usize> {
    let mut rows = store.0.write().unwrap();
    let mut n = 0;
    for r in rows.iter_mut().filter(|r| ids.contains(&r.id)) {
        r.active = false;
        n += 1;
    }
    Ok(n)
}

#[screen(route = "/pick", permission = "items.read")]
pub fn pick() -> Screen {
    Screen::new("Pick").body(
        Grid::of(search).paging(Paging::pages(5)).column(Item::code).column(Item::kind).filter().column(Item::active).bulk_action(
            "Deactivate",
            deactivate,
            Item::id,
        ),
    )
}

fn pick_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("items").screen(pick))
        .router()
        .unwrap();
    (router, store)
}

fn bulk(body: serde_json::Value) -> Request<Body> {
    let mut call = serde_json::json!({ "screen": "pick", "node": 0, "action": 0 });
    call.as_object_mut().unwrap().extend(body.as_object().unwrap().clone());
    post("/_zeyn/grid/bulk", call)
}

fn active_ids(store: &Store) -> Vec<i64> {
    store.0.read().unwrap().iter().filter(|r| r.active).map(|r| r.id.0).collect()
}

fn writer_items() -> Identity {
    Identity::user("w").with_permissions(["items.read", "items.write"])
}

#[tokio::test]
async fn selectable_grid_renders_checkboxes_and_a_bar_with_permitted_actions_only() {
    let (router, _) = pick_router(writer_items());
    let r = send(&router, get("/pick")).await;
    assert!(r.body.contains("data-zeyn-select-all"));
    assert!(r.body.contains(r#"<td class="z-select"><input type="checkbox" data-zeyn-select"#));
    assert!(r.body.contains(r#"data-zeyn-bulk-run="0""#));
    assert!(r.body.contains(r#"aria-colcount="4""#));
    let (router, _) = pick_router(reader());
    let r = send(&router, get("/pick")).await;
    assert!(r.body.contains("data-zeyn-bulk "));
    assert!(!r.body.contains("data-zeyn-bulk-run="));
}

#[tokio::test]
async fn bulk_action_runs_with_picked_keys() {
    let (router, store) = pick_router(writer_items());
    let r = send(&router, bulk(serde_json::json!({ "keys": ["2", "4", "4"] }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["count"], 2);
    assert!(!active_ids(&store).contains(&2) && !active_ids(&store).contains(&4));
    assert_eq!(active_ids(&store).len(), 8);
}

#[tokio::test]
async fn bulk_action_on_every_matching_row_collects_keys_on_the_server() {
    let (router, store) = pick_router(writer_items());
    let q = serde_json::json!({ "all": true, "keys": ["1"], "query": { "filters": [{ "field": "kind", "any_of": ["Tool"] }, { "field": "note", "contains": "zzz" }] } });
    let r = send(&router, bulk(q)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["count"], 6, "every tool, the note filter is not offered");
    assert_eq!(active_ids(&store), [2, 4, 8, 10, 14, 16, 20]);
}

#[tokio::test]
async fn bulk_action_is_refused_without_permission_bad_keys_unknown_actions_or_too_many() {
    let (router, store) = pick_router(reader());
    assert_eq!(send(&router, bulk(serde_json::json!({ "keys": ["2"] }))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(active_ids(&store).len(), 10);
    let (router, store) = pick_router(writer_items());
    assert_eq!(send(&router, bulk(serde_json::json!({ "keys": ["x"] }))).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, bulk(serde_json::json!({ "action": 3, "keys": ["2"] }))).await.status, StatusCode::NOT_FOUND);
    let many: Vec<String> = (0..=zeyn::MAX_BULK).map(|i| i.to_string()).collect();
    assert_eq!(send(&router, bulk(serde_json::json!({ "keys": many }))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(active_ids(&store).len(), 10);
}

#[action(permission = "items.admin")]
pub async fn wipe(id: ItemId, store: Store) -> Result<()> {
    store.0.write().unwrap().retain(|r| r.id != id);
    Ok(())
}

#[action(permission = "items.write")]
pub async fn bump_all(_: (), store: Store) -> Result<()> {
    for r in store.0.write().unwrap().iter_mut() {
        r.qty += 1;
    }
    Ok(())
}

#[screen(route = "/menus", permission = "items.read")]
pub fn menus() -> Screen {
    Screen::new("Menus")
        .context_menu(ContextMenu::new().item("Open", Effect::Toast { message: "hi".into() }).action("Bump all", bump_all, ()))
        .body(Grid::of(search).paging(Paging::pages(5)).column(Item::code).column(Item::qty).row_menu(|i| {
            ContextMenu::new()
                .item("Toast", Effect::Toast { message: "x".into() })
                .separator()
                .action("Bump", bump, i.id)
                .disabled(i.qty > 3)
                .submenu("Danger", ContextMenu::new().action("Wipe", wipe, i.id))
                .separator()
        }))
}

fn menu_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("items").screen(menus))
        .router()
        .unwrap();
    (router, store)
}

fn menu_run(key: &str, item: &str) -> Request<Body> {
    post("/_zeyn/grid/run", serde_json::json!({ "screen": "menus", "node": 0, "key": key, "item": item }))
}

#[tokio::test]
async fn row_menus_leave_out_items_the_user_may_not_run() {
    let (router, _) = menu_router(writer_items());
    let r = send(&router, get("/menus")).await;
    let first = r.body.split(r#"<tr data-zeyn-key="1" data-zeyn-menu=""#).nth(1).unwrap().split('"').next().unwrap().replace("&quot;", "\"");
    let items: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 3, "toast, separator, bump; no empty submenu, no dangling separator: {items}");
    assert_eq!(items[2]["run"], "2");
    let r = send(&router, get("/menus")).await;
    assert!(r.body.contains(r#"data-zeyn-screen="menus" data-zeyn-params="null" data-zeyn-menu=""#));
    let admin = Identity::user("a").with_permissions(["items.read", "items.write", "items.admin"]);
    let (router, _) = menu_router(admin);
    let r = send(&router, get("/menus")).await;
    assert!(r.body.contains("Wipe"));
}

#[tokio::test]
async fn row_menu_actions_run_from_the_row_on_the_server_and_check_permission_again() {
    let (router, store) = menu_router(writer_items());
    let r = send(&router, menu_run("2", "m2")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(store.0.read().unwrap()[1].qty, 102);
    assert_eq!(send(&router, menu_run("2", "m2")).await.status, StatusCode::NOT_FOUND, "disabled now that qty > 3");
    assert_eq!(send(&router, menu_run("1", "m3.0")).await.status, StatusCode::FORBIDDEN, "admin only");
    assert_eq!(store.0.read().unwrap().len(), 20);
    assert_eq!(send(&router, menu_run("1", "m0")).await.status, StatusCode::NOT_FOUND, "effects run in the browser");
    assert_eq!(send(&router, menu_run("1", "m9")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, menu_run("1", "m3")).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn screen_menu_actions_run_with_permission_checked() {
    let (router, store) = menu_router(writer_items());
    let call = |item: &str| post("/_zeyn/menu", serde_json::json!({ "screen": "menus", "item": item }));
    assert_eq!(send(&router, call("1")).await.status, StatusCode::OK);
    assert_eq!(store.0.read().unwrap()[0].qty, 2);
    assert_eq!(send(&router, call("0")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, call("7")).await.status, StatusCode::NOT_FOUND);
    let (router, store) = menu_router(reader());
    assert_eq!(send(&router, call("1")).await.status, StatusCode::FORBIDDEN);
    assert_eq!(store.0.read().unwrap()[0].qty, 1);
}

#[tokio::test]
async fn browser_menu_stays_unless_the_app_turns_it_off() {
    let (router, _) = menu_router(reader());
    assert!(!send(&router, get("/menus")).await.body.contains("data-zeyn-native-menu"));
    let router = App::new("Test")
        .service(Store::seeded())
        .browser_context_menu(false)
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(reader))
        .feature(Feature::new("items").screen(menus))
        .router()
        .unwrap();
    assert!(send(&router, get("/menus")).await.body.contains(r#"data-zeyn-native-menu="off""#));
}

#[action(permission = "items.write")]
pub async fn save_item(item: Valid<Item>, store: Store) -> Result<Item> {
    let mut rows = store.0.write().unwrap();
    let current = rows.iter_mut().find(|r| r.id == item.id).ok_or(Error::NotFound)?;
    if current.version != item.version {
        return Err(Error::Conflict);
    }
    let mut saved = item.into_inner();
    saved.version += 1;
    *current = saved.clone();
    Ok(saved)
}

#[screen(route = "/edit", permission = "items.read")]
pub fn edit_list() -> Screen {
    Screen::new("Edit").body(
        Grid::of(search)
            .paging(Paging::pages(5))
            .column(Item::code)
            .editable()
            .column(Item::price)
            .editable()
            .column(Item::qty)
            .column(Item::note)
            .editable()
            .column(Item::active)
            .editable()
            .edit_with(save_item),
    )
}

fn edit_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .i18n(I18n::new("en").catalog("tr", "{}").unwrap())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("items").screen(edit_list))
        .router()
        .unwrap();
    (router, store)
}

fn edit(key: &str, values: serde_json::Value) -> Request<Body> {
    post("/_zeyn/grid/edit", serde_json::json!({ "screen": "edit_list", "node": 0, "key": key, "values": values }))
}

#[tokio::test]
async fn editable_cells_carry_input_values_only_for_users_who_may_save() {
    let (router, _) = edit_router(writer_items());
    let r = send(&router, get("/edit")).await;
    assert!(r.body.contains(r#"data-field="code" aria-sort="none" aria-colindex="1" data-edit="text""#), "{}", r.body);
    assert!(r.body.contains(r#"data-zeyn-version-field="version""#));
    assert!(r.body.contains(r#"<tr data-zeyn-key="1" data-zeyn-version="1"><td data-v="C-01">C-01</td><td class="z-num" data-v="10.00">"#));
    assert!(!r.body.contains(r#"data-field="note" aria-sort="none" aria-colindex="4" data-edit"#), "read only");
    let (router, _) = edit_router(reader());
    let r = send(&router, get("/edit")).await;
    assert!(!r.body.contains("data-edit=") && !r.body.contains("data-v="));
}

#[tokio::test]
async fn inline_edit_saves_through_valid_and_returns_the_row() {
    let (router, store) = edit_router(writer_items());
    let r = send(&router, edit("3", serde_json::json!({ "code": "NEW-3", "price": "12.50", "active": true, "version": "1" }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(r.json()["row"].as_str().unwrap().contains(r#"data-zeyn-version="2"><td data-v="NEW-3">NEW-3</td>"#));
    let saved = store.0.read().unwrap()[2].clone();
    assert_eq!((saved.code.as_str(), saved.price, saved.active, saved.qty, saved.version), ("NEW-3", Money::tl(1250), true, 3, 2));
}

#[tokio::test]
async fn inline_edit_reports_validation_and_conflicts() {
    let (router, store) = edit_router(writer_items());
    let r = send(&router, edit("3", serde_json::json!({ "code": "", "price": "abc", "version": "1" }))).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: Vec<String> = r.json()["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap().to_string()).collect();
    assert_eq!(fields, ["price", "code"]);
    let r = send(&router, edit("3", serde_json::json!({ "code": "X", "version": "7" }))).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(store.0.read().unwrap()[2].code, "C-03");
}

#[tokio::test]
async fn inline_edit_refuses_read_only_and_unknown_columns_and_missing_permission() {
    let (router, store) = edit_router(writer_items());
    for values in [
        serde_json::json!({ "note": "hacked", "version": "1" }),
        serde_json::json!({ "qty": "999", "version": "1" }),
        serde_json::json!({ "id": "77", "code": "X", "version": "1" }),
        serde_json::json!({ "secret": "1" }),
        serde_json::json!({}),
    ] {
        assert_eq!(send(&router, edit("3", values)).await.status, StatusCode::FORBIDDEN);
    }
    assert_eq!(send(&router, edit("999", serde_json::json!({ "code": "X" }))).await.status, StatusCode::NOT_FOUND);
    let (router, _) = edit_router(reader());
    assert_eq!(send(&router, edit("3", serde_json::json!({ "code": "X", "version": "1" }))).await.status, StatusCode::FORBIDDEN);
    let row = store.0.read().unwrap()[2].clone();
    assert_eq!((row.note.as_str(), row.qty, row.code.as_str()), ("note 3", 3, "C-03"));
}
