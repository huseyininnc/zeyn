//! DataGrid features end to end: totals, filters, sorting, views, row actions, selection, editing
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
pub async fn search(q: DataQuery, store: Store) -> Result<Page<Item>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[screen(route = "/items", permission = "items.read")]
pub fn list() -> Screen {
    Screen::new("Items").body(
        DataGrid::of(search)
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
        .i18n(
            I18n::new("en")
                .catalog("tr", "{}")
                .unwrap(),
        )
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(
            Feature::new("items")
                .screen(list)
                .screen(banded)
                .screen(rows_edit)
                .api(search),
        )
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
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    Reply {
        status,
        headers,
        body,
    }
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn post_as(path: &str, body: serde_json::Value, locale: &str) -> Request<Body> {
    Request::post(path)
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(
            header::COOKIE,
            format!("zeyn_csrf={TOKEN}; zeyn_locale={locale}"),
        )
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn post(path: &str, body: serde_json::Value) -> Request<Body> {
    post_as(path, body, "en")
}

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .header("x-user", "1")
        .body(Body::empty())
        .unwrap()
}

fn agg(field: &str, op: Aggregate) -> AggregateRequest {
    AggregateRequest {
        field: field.into(),
        op,
    }
}

fn query(json: serde_json::Value) -> DataQuery {
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
    let value = |field: &str, op: Aggregate| {
        page.totals
            .iter()
            .find(|t| t.field == field && t.op == op)
            .unwrap()
            .value
            .clone()
    };
    assert_eq!(value("qty", Aggregate::Sum), Value::Int(210));
    assert_eq!(value("qty", Aggregate::Avg), Value::Decimal(10.5));
    assert_eq!(value("qty", Aggregate::Min), Value::Int(1));
    assert_eq!(value("qty", Aggregate::Max), Value::Int(20));
    assert_eq!(
        value("price", Aggregate::Sum),
        Value::Money(Money::tl(210_000))
    );
    assert_eq!(
        value("price", Aggregate::Avg),
        Value::Money(Money::tl(10_500))
    );
    assert_eq!(value("due", Aggregate::Count), Value::Int(5));
    assert_eq!(
        value("added", Aggregate::Max),
        Value::Date(Date::new(2026, 1, 20).unwrap())
    );
}

#[test]
fn totals_ignore_unknown_fields_and_ops_that_do_not_fit() {
    let q = DataQuery {
        aggregates: vec![
            agg("nope", Aggregate::Sum),
            agg("code", Aggregate::Sum),
            agg("code", Aggregate::Count),
        ],
        ..DataQuery::default()
    };
    let page = q.apply((1..=3).map(item).collect());
    assert_eq!(page.totals.len(), 1);
    assert_eq!(page.totals[0].value, Value::Int(3));
}

#[test]
fn money_average_rounds_half_away_from_zero() {
    let values = [Value::Money(Money::tl(1)), Value::Money(Money::tl(2))].into_iter();
    assert_eq!(
        zeyn::total(values, Aggregate::Avg),
        Some(Value::Money(Money::tl(2)))
    );
    let values = [Value::Money(Money::tl(-1)), Value::Money(Money::tl(-2))].into_iter();
    assert_eq!(
        zeyn::total(values, Aggregate::Avg),
        Some(Value::Money(Money::tl(-2)))
    );
    let mixed = [
        Value::Money(Money::tl(1)),
        Value::Money(Money::new(1, zeyn::Currency::Usd)),
    ]
    .into_iter();
    assert_eq!(zeyn::total(mixed, Aggregate::Sum), None);
    assert_eq!(
        zeyn::total(std::iter::empty(), Aggregate::Avg),
        Some(Value::Null)
    );
}

#[tokio::test]
async fn page_renders_footer_totals_with_the_first_page() {
    let r = send(&router(reader()), get("/items")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        r.body
            .contains(r#"<tfoot data-zeyn-foot>"#)
    );
    assert!(
        r.body
            .contains(r#"data-field="qty" data-agg="sum avg min max""#)
    );
    assert!(
        r.body.contains(
            r#"<span class="z-agg-label">Sum</span> <span class="z-agg-value">210</span>"#
        )
    );
    assert!(r.body.contains("₺2,100.00"));
    assert!(
        r.body
            .contains(r#"<span class="z-agg-value">10.50</span>"#)
    );
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
    let r = send(
        &router(reader()),
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "list", "node": 0 }),
        ),
    )
    .await;
    assert!(r.json()["foot"].is_null());
}

#[tokio::test]
async fn grid_endpoint_needs_the_read_permission() {
    let r = send(
        &router(Identity::user("x")),
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "list", "node": 0 }),
        ),
    )
    .await;
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
    page.items
        .iter()
        .map(|i| i.code.clone())
        .collect()
}

fn filtered(filters: Vec<zeyn::Filter>) -> Page<Item> {
    let q = DataQuery {
        filters,
        page: zeyn::PageRequest::Offset {
            offset: 0,
            limit: 100,
        },
        ..DataQuery::default()
    };
    q.apply((1..=20).map(item).collect())
}

#[test]
fn number_and_money_ranges_are_inclusive_and_open_ended() {
    use zeyn::Filter;
    assert_eq!(
        codes(&filtered(vec![Filter::range("qty", Some("18"), None)])),
        ["C-18", "C-19", "C-20"]
    );
    assert_eq!(
        codes(&filtered(vec![Filter::range(
            "price",
            Some("10"),
            Some("30.00")
        )])),
        ["C-01", "C-02", "C-03"]
    );
    assert_eq!(
        filtered(vec![Filter::range("price", Some("10,5"), Some("20"))])
            .items
            .len(),
        1
    );
    assert_eq!(
        filtered(vec![Filter::range("qty", Some("abc"), Some("1e9"))])
            .items
            .len(),
        20
    );
}

#[test]
fn date_ranges_and_optional_dates() {
    use zeyn::Filter;
    let page = filtered(vec![Filter::range(
        "added",
        Some("2026-01-05"),
        Some("2026-01-07"),
    )]);
    assert_eq!(codes(&page), ["C-05", "C-06", "C-07"]);
    let page = filtered(vec![Filter::range("due", None, Some("2026-03-08"))]);
    assert_eq!(codes(&page), ["C-04", "C-08"]);
}

#[test]
fn moment_date_filters_use_the_users_whole_day() {
    use zeyn::Filter;
    let at = DateTime::parse("2026-03-01T22:30:00Z").unwrap();
    let rows = vec![Event { id: EventId(1), at }];
    let mut q = DataQuery {
        filters: vec![Filter::range("at", Some("2026-03-02"), Some("2026-03-02"))],
        ..DataQuery::default()
    };
    assert_eq!(q.apply(rows.clone()).items.len(), 0);
    q.time_zone = TimeZone::named("Europe/Istanbul").unwrap();
    assert_eq!(q.apply(rows.clone()).items.len(), 1);
    q.filters = vec![Filter::range(
        "at",
        Some("2026-03-02T01:00"),
        Some("2026-03-02T01:30"),
    )];
    assert_eq!(q.apply(rows).items.len(), 1);
}

#[test]
fn choices_booleans_and_text_operators() {
    use zeyn::Filter;
    assert_eq!(
        filtered(vec![Filter::any_of("kind", ["Tool", "nope"])])
            .items
            .len(),
        6
    );
    assert_eq!(
        filtered(vec![Filter::any_of("kind", ["nope"])])
            .items
            .len(),
        0
    );
    assert_eq!(
        filtered(vec![Filter::is("active", true)])
            .items
            .len(),
        10
    );
    let starts = Filter {
        field: "code".into(),
        starts_with: Some("c-1".into()),
        ..Filter::default()
    };
    assert_eq!(filtered(vec![starts]).items.len(), 10);
    let equals = Filter {
        field: "code".into(),
        equals: Some("C-07".into()),
        ..Filter::default()
    };
    assert_eq!(codes(&filtered(vec![equals])), ["C-07"]);
    let too_long = Filter::contains("code", "x".repeat(zeyn::MAX_FILTER_TEXT + 1));
    assert_eq!(filtered(vec![too_long]).items.len(), 20);
}

#[test]
fn old_contains_filters_still_read() {
    let q: DataQuery =
        query(serde_json::json!({ "filters": [{ "field": "code", "contains": "C-2" }] }));
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
    let rows = r.json()["rows"]
        .as_str()
        .unwrap()
        .to_string();
    let order: Vec<usize> = ["C-20", "C-17", "C-14", "C-11"]
        .iter()
        .map(|c| rows.find(c).unwrap())
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{rows}");
}

#[tokio::test]
async fn header_names_filter_kinds_and_localized_choices() {
    let req = Request::get("/items")
        .header("x-user", "1")
        .header(header::COOKIE, "zeyn_locale=tr")
        .body(Body::empty())
        .unwrap();
    let r = send(&router(reader()), req).await;
    assert!(
        r.body.contains(
            r#"data-field="price" aria-sort="none" aria-colindex="3" data-filter="money""#
        ),
        "{}",
        r.body
    );
    assert!(
        r.body
            .contains(r#"data-filter="choice" data-options=""#)
    );
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
            let user = h
                .get("x-user")?
                .to_str()
                .ok()?
                .to_string();
            let perms: Vec<&str> = if user == "guest" {
                vec![]
            } else {
                vec!["items.read"]
            };
            Some(Identity::user(user).with_permissions(perms))
        })
        .feature(Feature::new("items").screen(list))
        .router()
        .unwrap()
}

fn as_user(mut req: Request<Body>, user: &str) -> Request<Body> {
    req.headers_mut()
        .insert("x-user", user.parse().unwrap());
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
    let r = send(
        &router,
        as_user(post("/_zeyn/grid/view", save_view(view)), "ayse"),
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.body);

    let page = send(&router, as_user(get("/items"), "ayse")).await;
    assert!(
        page.body
            .contains(r#"data-zeyn-views="server""#)
    );
    let head = page
        .body
        .split("<thead>")
        .nth(1)
        .unwrap()
        .split("</thead>")
        .next()
        .unwrap();
    assert!(
        head.find(r#"data-field="code""#)
            .unwrap()
            < head
                .find(r#"data-field="qty""#)
                .unwrap()
    );
    assert!(head.contains(r#"data-pin="start""#));
    assert!(head.contains("width:1200px;"), "{head}");
    assert!(!head.contains(r#"data-field="kind""#));
    assert!(
        page.body
            .contains(r#"data-zeyn-total="6""#),
        "only tools, the note filter is not offered"
    );
    let rows = page
        .body
        .split("<tbody data-zeyn-rows>")
        .nth(1)
        .unwrap();
    assert!(
        rows.starts_with("<tr data-zeyn-key=\"18\"><td class=\"z-pin z-ps1 z-pin-edge-s\">C-18</td><td class=\"z-num\">18</td></tr>"),
        "{}",
        &rows[..120]
    );

    let other = send(&router, as_user(get("/items"), "mert")).await;
    assert!(!other.body.contains("data-zeyn-view="));
    assert!(
        other
            .body
            .contains(r#"data-zeyn-total="20""#)
    );

    let r = send(
        &router,
        as_user(
            post("/_zeyn/grid/view", save_view(serde_json::Value::Null)),
            "ayse",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let page = send(&router, as_user(get("/items"), "ayse")).await;
    assert!(!page.body.contains("data-zeyn-view="));
}

#[tokio::test]
async fn saving_a_view_needs_a_store_a_user_and_read_access() {
    let views = views_router(zeyn::MemoryViewStore::default());
    let r = send(
        &views,
        as_user(
            post("/_zeyn/grid/view", save_view(serde_json::json!({}))),
            "guest",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = send(
        &router(reader()),
        post("/_zeyn/grid/view", save_view(serde_json::json!({}))),
    )
    .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(
        &views,
        as_user(
            post(
                "/_zeyn/grid/view",
                serde_json::json!({ "screen": "list", "node": 7, "view": {} }),
            ),
            "ayse",
        ),
    )
    .await;
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
    assert_eq!(
        j["rows"],
        "<tr data-zeyn-key=\"1\"><td class=\"z-num\">1</td><td class=\"z-num z-pin z-pe1 z-pin-edge-e\">₺10.00</td></tr>"
    );
    let head = j["head"].as_str().unwrap();
    assert!(head.find("qty").unwrap() < head.find("price").unwrap());
    assert!(head.contains(r#"data-pin="end""#));
    let foot = j["foot"].as_str().unwrap();
    assert!(
        foot.find(">210<").unwrap() < foot.find("₺2,100.00").unwrap(),
        "{foot}"
    );
}

#[test]
fn at_most_three_columns_stay_pinned_per_side() {
    use zeyn::{DataGridView, GridSource};
    let grid = DataGrid::of(search)
        .column(Item::code)
        .column(Item::kind)
        .column(Item::price)
        .column(Item::qty)
        .column(Item::added)
        .done();
    let columns: Vec<serde_json::Value> = ["code", "kind", "price", "qty", "added"]
        .iter()
        .map(|f| serde_json::json!({ "field": f, "pin": "start" }))
        .collect();
    let view: DataGridView =
        serde_json::from_value(serde_json::json!({ "columns": columns })).unwrap();
    let clean = GridSource::clean_view(&grid, view);
    assert_eq!(
        clean
            .columns
            .iter()
            .filter(|c| c.pin.is_some())
            .count(),
        3
    );
    assert_eq!(clean.columns.len(), 5);
}

#[action(permission = "items.write")]
pub async fn bump(id: ItemId, store: Store) -> Result<()> {
    let mut rows = store.0.write().unwrap();
    let row = rows
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or(Error::NotFound)?;
    if row.qty > 100 {
        return Err(Error::Denied("too many".into()));
    }
    row.qty += 100;
    Ok(())
}

#[screen(route = "/act", permission = "items.read")]
pub fn act() -> Screen {
    Screen::new("Act").body(
        DataGrid::of(search)
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

#[screen(route = "/act-status", permission = "items.read")]
pub fn act_status() -> Screen {
    Screen::new("Act status").body(
        DataGrid::of(search)
            .zebra()
            .column(Item::code)
            .wrap()
            .column(Item::kind)
            .status(|k| match k {
                Kind::Tool => zeyn::Tone::Warning,
                _ => zeyn::Tone::Success,
            }),
    )
}

#[screen(route = "/act-nested", permission = "items.read")]
pub fn act_nested() -> Screen {
    Screen::new("Act nested").body(
        Panel::new("Items").add(
            DataGrid::of(search)
                .column(Item::code)
                .row_action("Bump", bump, |i| i.id),
        ),
    )
}

#[screen(route = "/sorted", permission = "items.read")]
pub fn sorted() -> Screen {
    Screen::new("Sorted").body(
        DataGrid::of(search)
            .sort_by_desc(Item::qty)
            .column(Item::code)
            .column(Item::qty),
    )
}

fn act_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(
            Feature::new("items")
                .screen(act)
                .screen(act_status)
                .screen(act_nested)
                .screen(sorted),
        )
        .router()
        .unwrap();
    (router, store)
}

fn run(key: &str, item: &str) -> Request<Body> {
    post(
        "/_zeyn/grid/run",
        serde_json::json!({ "screen": "act", "node": 0, "key": key, "item": item }),
    )
}

#[test]
fn pinned_cells_are_found_by_class_not_by_position() {
    let css = include_str!("../assets/zeyn.css");
    assert!(
        !css.contains(":nth-child(-n +") && !css.contains("tr > :nth-child("),
        "positional rules match every cell of every grid and cost about a third of style time"
    );
    for class in [".z-ps1 {", ".z-ps5 {", ".z-pe3 {", ":where(.z-grid .z-pin)"] {
        assert!(css.contains(class), "{class}");
    }
}

#[tokio::test]
async fn status_cells_draw_a_dot_and_zebra_and_wrap_are_opt_in() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/act-status")).await;
    assert!(
        r.body
            .contains(r#"<span class="z-badge z-badge-dot z-tone-warning">Tool</span>"#)
    );
    assert!(
        r.body
            .contains(r#"<span class="z-badge z-badge-dot z-tone-success">Part</span>"#)
    );
    assert!(
        r.body
            .contains(r#"<div class="z-grid z-zebra" data-zeyn-grid"#)
    );
    assert!(
        r.body
            .contains(r#"<td class="z-wrap">"#)
    );
    let plain = send(&router, get("/act")).await;
    assert!(
        plain
            .body
            .contains(r#"<div class="z-grid" data-zeyn-grid"#)
            && !plain.body.contains("z-wrap"),
        "zebra rows and wrapping are opt in"
    );
}

#[tokio::test]
async fn cells_render_badges_progress_and_icons() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/act")).await;
    assert!(
        r.body
            .contains(r#"<span class="z-badge z-tone-neutral">Part</span>"#)
    );
    assert!(
        r.body
            .contains(r#"<span class="z-badge z-tone-info">Tool</span>"#)
    );
    assert!(
        r.body
            .contains(r#"style="--z-pct:2.5%""#),
        "1 of 40"
    );
    assert!(
        r.body
            .contains(r#"<span class="z-cell-progress"><span class="z-cell-progress-track""#),
        "grid cells have classes of their own, so the Progress widget's layout cannot squeeze them"
    );
    let css = include_str!("../assets/zeyn.css");
    assert!(
        css.contains(".z-button[hidden] { display: none; }"),
        "a hidden button, such as a card grid's load more without more rows, stays hidden"
    );
    assert!(
        r.body
            .contains(r#"<span class="z-bool z-bool-no" role="img" aria-label="No"></span>"#)
    );
    assert!(
        r.body
            .contains(r#"<tr data-zeyn-key="1">"#)
    );
}

#[tokio::test]
async fn row_action_buttons_show_only_with_the_actions_permission() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/act")).await;
    assert!(
        r.body
            .contains(r#"<td class="z-row-actions z-pin z-pe1 z-pin-edge-e"></td>"#)
    );
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
    assert_eq!(
        r.status,
        StatusCode::FORBIDDEN,
        "the action's own error comes back"
    );
}

#[tokio::test]
async fn a_default_sort_orders_the_first_page_and_reaches_the_browser() {
    let (router, _) = act_router(reader());
    let r = send(&router, get("/sorted")).await;
    assert!(
        r.body.contains(r#"data-zeyn-sort-default="[{&quot;field&quot;:&quot;qty&quot;,&quot;desc&quot;:true}]""#),
        "{}",
        r.body
    );
    let first = r
        .body
        .find(r#"<tr data-zeyn-key="#)
        .map(|i| &r.body[i..i + 30])
        .unwrap_or_default();
    let max = Store::seeded()
        .0
        .read()
        .unwrap()
        .iter()
        .max_by_key(|i| i.qty)
        .map(|i| i.id.0)
        .unwrap();
    assert!(
        first.contains(&format!(r#"data-zeyn-key="{max}""#)),
        "{first}"
    );
}

#[tokio::test]
async fn grid_endpoints_take_node_paths_as_the_browser_sends_them() {
    let writer = Identity::user("w").with_permissions(["items.read", "items.write"]);
    let (router, store) = act_router(writer);
    let top = post(
        "/_zeyn/grid/run",
        serde_json::json!({ "screen": "act", "node": "0", "key": "7", "item": "a0" }),
    );
    assert_eq!(send(&router, top).await.status, StatusCode::OK);
    let page = send(&router, get("/act-nested")).await;
    assert!(
        page.body
            .contains(r#"data-zeyn-node="0.0""#),
        "{}",
        page.body
    );
    let nested = post(
        "/_zeyn/grid/run",
        serde_json::json!({ "screen": "act_nested", "node": "0.0", "key": "8", "item": "a0" }),
    );
    assert_eq!(send(&router, nested).await.status, StatusCode::OK);
    assert_eq!(
        (
            store.0.read().unwrap()[6].qty,
            store.0.read().unwrap()[7].qty
        ),
        (107, 108)
    );
    let missing = post(
        "/_zeyn/grid/run",
        serde_json::json!({ "screen": "act_nested", "node": "0.5", "key": "7", "item": "a0" }),
    );
    assert_eq!(send(&router, missing).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn row_action_is_refused_without_permission_unknown_items_or_rows() {
    let (router, store) = act_router(reader());
    assert_eq!(
        send(&router, run("7", "a0"))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(store.0.read().unwrap()[6].qty, 7);
    let writer = Identity::user("w").with_permissions(["items.read", "items.write"]);
    let (router, _) = act_router(writer.clone());
    assert_eq!(
        send(&router, run("7", "a1"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&router, run("7", "x"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&router, run("999", "a0"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let (router, _) = act_router(Identity::user("w").with_permissions(["items.write"]));
    assert_eq!(
        send(&router, run("7", "a0"))
            .await
            .status,
        StatusCode::FORBIDDEN,
        "screen access first"
    );
}

#[action(permission = "items.write")]
pub async fn deactivate(ids: Vec<ItemId>, store: Store) -> Result<usize> {
    let mut rows = store.0.write().unwrap();
    let mut n = 0;
    for r in rows
        .iter_mut()
        .filter(|r| ids.contains(&r.id))
    {
        r.active = false;
        n += 1;
    }
    Ok(n)
}

#[screen(route = "/pick", permission = "items.read")]
pub fn pick() -> Screen {
    Screen::new("Pick").body(
        DataGrid::of(search)
            .paging(Paging::pages(5))
            .column(Item::code)
            .column(Item::kind)
            .filter()
            .column(Item::active)
            .bulk_action("Deactivate", deactivate, Item::id),
    )
}

fn pick_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(pick))
        .router()
        .unwrap();
    (router, store)
}

fn bulk(body: serde_json::Value) -> Request<Body> {
    let mut call = serde_json::json!({ "screen": "pick", "node": 0, "action": 0 });
    call.as_object_mut()
        .unwrap()
        .extend(body.as_object().unwrap().clone());
    post("/_zeyn/grid/bulk", call)
}

fn active_ids(store: &Store) -> Vec<i64> {
    store
        .0
        .read()
        .unwrap()
        .iter()
        .filter(|r| r.active)
        .map(|r| r.id.0)
        .collect()
}

fn writer_items() -> Identity {
    Identity::user("w").with_permissions(["items.read", "items.write"])
}

#[tokio::test]
async fn selectable_grid_renders_checkboxes_and_a_bar_with_permitted_actions_only() {
    let (router, _) = pick_router(writer_items());
    let r = send(&router, get("/pick")).await;
    assert!(r.body.contains("data-zeyn-select-all"));
    assert!(
        r.body
            .contains(r#"<td class="z-row-select z-pin z-ps1 z-pin-edge-s"><input type="checkbox" data-zeyn-select"#)
    );
    assert!(
        r.body
            .contains(r#"data-zeyn-bulk-run="0""#)
    );
    assert!(r.body.contains(r#"aria-colcount="4""#));
    let (router, _) = pick_router(reader());
    let r = send(&router, get("/pick")).await;
    assert!(r.body.contains("data-zeyn-bulk "));
    assert!(!r.body.contains("data-zeyn-bulk-run="));
}

#[tokio::test]
async fn bulk_action_runs_with_picked_keys() {
    let (router, store) = pick_router(writer_items());
    let r = send(
        &router,
        bulk(serde_json::json!({ "keys": ["2", "4", "4"] })),
    )
    .await;
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
    assert_eq!(
        r.json()["count"],
        6,
        "every tool, the note filter is not offered"
    );
    assert_eq!(active_ids(&store), [2, 4, 8, 10, 14, 16, 20]);
}

#[tokio::test]
async fn bulk_action_is_refused_without_permission_bad_keys_unknown_actions_or_too_many() {
    let (router, store) = pick_router(reader());
    assert_eq!(
        send(&router, bulk(serde_json::json!({ "keys": ["2"] })))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(active_ids(&store).len(), 10);
    let (router, store) = pick_router(writer_items());
    assert_eq!(
        send(&router, bulk(serde_json::json!({ "keys": ["x"] })))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &router,
            bulk(serde_json::json!({ "action": 3, "keys": ["2"] }))
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let many: Vec<String> = (0..=zeyn::MAX_BULK)
        .map(|i| i.to_string())
        .collect();
    assert_eq!(
        send(&router, bulk(serde_json::json!({ "keys": many })))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(active_ids(&store).len(), 10);
}

#[action(permission = "items.admin")]
pub async fn wipe(id: ItemId, store: Store) -> Result<()> {
    store
        .0
        .write()
        .unwrap()
        .retain(|r| r.id != id);
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
        .context_menu(
            ContextMenu::new()
                .item(
                    "Open",
                    Effect::Toast {
                        message: "hi".into(),
                    },
                )
                .action("Bump all", bump_all, ()),
        )
        .body(
            DataGrid::of(search)
                .paging(Paging::pages(5))
                .column(Item::code)
                .column(Item::qty)
                .row_menu(|i| {
                    ContextMenu::new()
                        .item(
                            "Toast",
                            Effect::Toast {
                                message: "x".into(),
                            },
                        )
                        .separator()
                        .action("Bump", bump, i.id)
                        .disabled(i.qty > 3)
                        .submenu("Danger", ContextMenu::new().action("Wipe", wipe, i.id))
                        .separator()
                }),
        )
}

fn menu_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    let router = App::new("Test")
        .service(store.clone())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(menus))
        .router()
        .unwrap();
    (router, store)
}

fn menu_run(key: &str, item: &str) -> Request<Body> {
    post(
        "/_zeyn/grid/run",
        serde_json::json!({ "screen": "menus", "node": 0, "key": key, "item": item }),
    )
}

#[tokio::test]
async fn row_menus_leave_out_items_the_user_may_not_run() {
    let (router, _) = menu_router(writer_items());
    let r = send(&router, get("/menus")).await;
    let first = r
        .body
        .split(r#"<tr data-zeyn-key="1" data-zeyn-menu=""#)
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .replace("&quot;", "\"");
    let items: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(
        items.as_array().unwrap().len(),
        3,
        "toast, separator, bump; no empty submenu, no dangling separator: {items}"
    );
    assert_eq!(items[2]["run"], "2");
    let r = send(&router, get("/menus")).await;
    assert!(
        r.body
            .contains(r#"data-zeyn-screen="menus" data-zeyn-params="null" data-zeyn-menu=""#)
    );
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
    assert_eq!(
        send(&router, menu_run("2", "m2"))
            .await
            .status,
        StatusCode::NOT_FOUND,
        "disabled now that qty > 3"
    );
    assert_eq!(
        send(&router, menu_run("1", "m3.0"))
            .await
            .status,
        StatusCode::FORBIDDEN,
        "admin only"
    );
    assert_eq!(store.0.read().unwrap().len(), 20);
    assert_eq!(
        send(&router, menu_run("1", "m0"))
            .await
            .status,
        StatusCode::NOT_FOUND,
        "effects run in the browser"
    );
    assert_eq!(
        send(&router, menu_run("1", "m9"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&router, menu_run("1", "m3"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn screen_menu_actions_run_with_permission_checked() {
    let (router, store) = menu_router(writer_items());
    let call = |item: &str| {
        post(
            "/_zeyn/menu",
            serde_json::json!({ "screen": "menus", "item": item }),
        )
    };
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
    assert!(
        !send(&router, get("/menus"))
            .await
            .body
            .contains("data-zeyn-native-menu")
    );
    let router = App::new("Test")
        .service(Store::seeded())
        .no_browser_context_menu()
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(reader))
        .feature(Feature::new("items").screen(menus))
        .router()
        .unwrap();
    assert!(
        send(&router, get("/menus"))
            .await
            .body
            .contains(r#"data-zeyn-native-menu="off""#)
    );
}

#[action(permission = "items.write")]
pub async fn save_item(item: Valid<Item>, store: Store) -> Result<Item> {
    let mut rows = store.0.write().unwrap();
    let current = rows
        .iter_mut()
        .find(|r| r.id == item.id)
        .ok_or(Error::NotFound)?;
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
        DataGrid::of(search)
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
        .i18n(
            I18n::new("en")
                .catalog("tr", "{}")
                .unwrap(),
        )
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(edit_list))
        .router()
        .unwrap();
    (router, store)
}

fn edit(key: &str, values: serde_json::Value) -> Request<Body> {
    post(
        "/_zeyn/grid/edit",
        serde_json::json!({ "screen": "edit_list", "node": 0, "key": key, "values": values }),
    )
}

#[tokio::test]
async fn editable_cells_carry_input_values_only_for_users_who_may_save() {
    let (router, _) = edit_router(writer_items());
    let r = send(&router, get("/edit")).await;
    assert!(
        r.body
            .contains(r#"data-field="code" aria-sort="none" aria-colindex="1" data-edit="text""#),
        "{}",
        r.body
    );
    assert!(
        r.body
            .contains(r#"data-zeyn-version-field="version""#)
    );
    assert!(r.body.contains(
        r#"<tr data-zeyn-key="1" data-zeyn-version="1"><td data-v="C-01">C-01</td><td class="z-num" data-v="10.00">"#
    ));
    assert!(
        !r.body
            .contains(r#"data-field="note" aria-sort="none" aria-colindex="4" data-edit"#),
        "read only"
    );
    let (router, _) = edit_router(reader());
    let r = send(&router, get("/edit")).await;
    assert!(!r.body.contains("data-edit=") && !r.body.contains("data-v="));
}

#[tokio::test]
async fn inline_edit_saves_through_valid_and_returns_the_row() {
    let (router, store) = edit_router(writer_items());
    let r = send(
        &router,
        edit("3", serde_json::json!({ "code": "NEW-3", "price": "12.50", "active": true, "version": "1" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(
        r.json()["row"]
            .as_str()
            .unwrap()
            .contains(r#"data-zeyn-version="2"><td data-v="NEW-3">NEW-3</td>"#)
    );
    let saved = store.0.read().unwrap()[2].clone();
    assert_eq!(
        (
            saved.code.as_str(),
            saved.price,
            saved.active,
            saved.qty,
            saved.version
        ),
        ("NEW-3", Money::tl(1250), true, 3, 2)
    );
}

#[tokio::test]
async fn inline_edit_reports_validation_and_conflicts() {
    let (router, store) = edit_router(writer_items());
    let r = send(
        &router,
        edit(
            "3",
            serde_json::json!({ "code": "", "price": "abc", "version": "1" }),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let fields: Vec<String> = r.json()["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["field"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(fields, ["price", "code"]);
    let r = send(
        &router,
        edit("3", serde_json::json!({ "code": "X", "version": "7" })),
    )
    .await;
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
        assert_eq!(
            send(&router, edit("3", values))
                .await
                .status,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        send(&router, edit("999", serde_json::json!({ "code": "X" })))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let (router, _) = edit_router(reader());
    assert_eq!(
        send(
            &router,
            edit("3", serde_json::json!({ "code": "X", "version": "1" }))
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    let row = store.0.read().unwrap()[2].clone();
    assert_eq!(
        (row.note.as_str(), row.qty, row.code.as_str()),
        ("note 3", 3, "C-03")
    );
}

#[screen(route = "/export", permission = "items.read")]
pub fn export_list() -> Screen {
    Screen::new("Export")
        .body(
            DataGrid::of(search)
                .paging(Paging::pages(5))
                .column(Item::code)
                .filter()
                .column(Item::kind)
                .column(Item::price)
                .column(Item::active)
                .exportable(),
        )
        .body(
            DataGrid::of(search)
                .column(Item::code)
                .done()
                .export_permission("items.export")
                .export_limit(7),
        )
        .body(DataGrid::of(search).column(Item::code))
}

fn export_router(identity: Identity) -> (Router, Store) {
    let store = Store::seeded();
    store.0.write().unwrap()[0].code = "=HYPERLINK(\"x\")".into();
    store.0.write().unwrap()[1].code = "x;=cmd|' /C calc'!A0\t+1;-2".into();
    let router = App::new("Test")
        .service(store.clone())
        .i18n(
            I18n::new("en")
                .catalog(
                    "tr",
                    r#"{ "Item": { "code": "Kod" }, "Kind": { "Tool": "Alet" } }"#,
                )
                .unwrap(),
        )
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(export_list))
        .router()
        .unwrap();
    (router, store)
}

fn export_get(node: usize, q: serde_json::Value, locale: &str) -> Request<Body> {
    let q = q.to_string();
    let enc: String = q
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    Request::get(format!(
        "/_zeyn/grid/export?screen=export_list&node={node}&q={enc}"
    ))
    .header("x-user", "1")
    .header(header::COOKIE, format!("zeyn_locale={locale}"))
    .body(Body::empty())
    .unwrap()
}

#[tokio::test]
async fn export_streams_every_matching_row_in_the_users_layout_and_language() {
    let (router, _) = export_router(reader());
    let q = serde_json::json!({ "filters": [{ "field": "code", "starts_with": "C-1" }], "sort": [{ "field": "price", "desc": true }], "columns": [{ "field": "price" }, { "field": "code" }, { "field": "kind" }, { "field": "nope" }] });
    let r = send(&router, export_get(0, q, "tr")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        r.headers
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    assert!(
        r.headers
            .get(header::CONTENT_DISPOSITION)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("export_list.csv")
    );
    let lines: Vec<&str> = r
        .body
        .trim_start_matches('\u{feff}')
        .lines()
        .collect();
    assert_eq!(lines[0], "Price,Kod,Kind");
    assert_eq!(
        lines.len(),
        11,
        "every C-1x row, not only the first page of 5"
    );
    assert_eq!(lines[1], "\"190,00 ₺\",C-19,Part");
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with(",Alet"))
    );
}

/// A source that pages only by keyset: an offset past the first row returns nothing, like a
/// data layer whose cursors carry row values.
#[action(permission = "items.read")]
pub async fn keyset_search(q: DataQuery) -> Result<Page<Item>> {
    const ROWS: i64 = 1_203;
    let after = match &q.page {
        zeyn::PageRequest::After {
            cursor: Some(c), ..
        } => {
            c.0.strip_prefix('k')
                .and_then(|n| n.parse::<i64>().ok())
                .unwrap_or(ROWS)
        }
        zeyn::PageRequest::Offset { offset: 0, .. }
        | zeyn::PageRequest::After { cursor: None, .. } => 0,
        zeyn::PageRequest::Offset { .. } => ROWS,
    };
    let end = (after + i64::from(q.limit())).min(ROWS);
    let page = Page::new(
        (after + 1..=end)
            .map(|i| Item {
                id: ItemId(i),
                qty: i,
                ..item(1)
            })
            .collect(),
    );
    Ok(if end < ROWS {
        page.next(zeyn::Cursor(format!("k{end}")))
    } else {
        page
    })
}

#[screen(route = "/keyset-export", permission = "items.read")]
pub fn keyset_export() -> Screen {
    Screen::new("Keyset export").body(
        DataGrid::of(keyset_search)
            .column(Item::qty)
            .exportable(),
    )
}

#[tokio::test]
async fn export_follows_keyset_cursors_past_the_first_batch() {
    let identity = reader();
    let router = App::new("Test")
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(keyset_export))
        .router()
        .unwrap();
    let r = send(
        &router,
        Request::get("/_zeyn/grid/export?screen=keyset_export&node=0")
            .header("x-user", "1")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let rows: Vec<&str> = r
        .body
        .trim_start_matches('\u{feff}')
        .lines()
        .skip(1)
        .collect();
    assert_eq!(rows.len(), 1_203, "every row once, across three batches");
    assert_eq!(rows[0], "1");
    assert_eq!(rows[1_202], "1203");
}

#[tokio::test]
async fn export_defuses_formulas_and_respects_the_limit_and_permissions() {
    let (router, _) = export_router(reader());
    let r = send(&router, export_get(0, serde_json::json!({}), "en")).await;
    assert!(
        r.body
            .contains("\"'=HYPERLINK(\"\"x\"\")\""),
        "{}",
        &r.body[..200]
    );
    assert!(
        r.body
            .contains("\"x;'=cmd|' /C calc'!A0\t'+1;-2\""),
        "every part is defused: {}",
        &r.body[..300]
    );
    assert_eq!(
        send(&router, export_get(1, serde_json::json!({}), "en"))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, export_get(2, serde_json::json!({}), "en"))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let (router, _) =
        export_router(Identity::user("x").with_permissions(["items.read", "items.export"]));
    let r = send(&router, export_get(1, serde_json::json!({}), "en")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body.lines().count(), 8, "header and the limit of 7 rows");
    let (router, _) = export_router(Identity::user("x"));
    assert_eq!(
        send(&router, export_get(0, serde_json::json!({}), "en"))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn exportable_grids_tell_the_runtime() {
    let (router, _) = export_router(reader());
    let r = send(&router, get("/export")).await;
    assert_eq!(
        r.body
            .matches(r#"data-zeyn-exportable="csv"#)
            .count(),
        1,
        "the second needs items.export"
    );
}

/// Which item a detail screen shows.
#[params]
pub struct ItemParams {
    pub id: ItemId,
}

#[screen(route = "/master", permission = "items.read")]
pub fn master() -> Screen {
    Screen::new("Master")
        .body(
            DataGrid::of(search)
                .paging(Paging::pages(3))
                .column(Item::code)
                .detail(|i| (item_detail, ItemParams { id: i.id })),
        )
        .body(
            DataGrid::of(search)
                .paging(Paging::virtual_scroll())
                .column(Item::code)
                .detail(|i| (item_detail, ItemParams { id: i.id })),
        )
}

#[screen(permission = "items.detail")]
pub fn item_detail(params: ItemParams) -> Screen {
    Screen::window("Detail").body(
        DataGrid::of(search)
            .scope(Item::id, params.id)
            .column(Item::code)
            .column(Item::qty),
    )
}

fn master_router(identity: Identity) -> Router {
    App::new("Test")
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(
            Feature::new("items")
                .screen(master)
                .screen(item_detail),
        )
        .router()
        .unwrap()
}

#[tokio::test]
async fn rows_expand_into_a_typed_detail_screen_only_for_users_who_may_open_it() {
    let router =
        master_router(Identity::user("d").with_permissions(["items.read", "items.detail"]));
    let r = send(&router, get("/master")).await;
    let paged = &r.body[..r
        .body
        .find(r#"data-zeyn-node="1""#)
        .unwrap()];
    assert!(
        paged.contains(
            r#"<th scope="col" class="z-expand-col z-pin z-ps1 z-pin-edge-s" data-pin="start">"#
        ),
        "{paged}"
    );
    assert!(paged.contains(r#"data-field="code" aria-sort="none" aria-colindex="2""#));
    assert!(paged.contains(r#"aria-expanded="false" data-zeyn-detail="{&quot;params&quot;:{&quot;id&quot;:1},&quot;screen&quot;:&quot;item_detail&quot;}""#), "{paged}");
    assert_eq!(
        paged
            .matches("data-zeyn-expand")
            .count(),
        3
    );
    let virtual_grid = &r.body[r
        .body
        .find(r#"data-zeyn-node="1""#)
        .unwrap()..];
    assert!(
        !virtual_grid.contains("data-zeyn-detail") && !virtual_grid.contains("z-expand"),
        "virtual grids have no details"
    );
    let router = master_router(reader());
    let r = send(&router, get("/master")).await;
    assert!(!r.body.contains("data-zeyn-detail") && !r.body.contains("z-expand-col"));
}

#[tokio::test]
async fn detail_screens_check_their_permission_and_keep_their_scope() {
    let router =
        master_router(Identity::user("d").with_permissions(["items.read", "items.detail"]));
    let r = send(
        &router,
        get("/_zeyn/screen/item_detail?params=%7B%22id%22%3A4%7D"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        r.body.contains("C-04") && !r.body.contains("C-05"),
        "{}",
        r.body
    );
    let mut filters: Vec<serde_json::Value> = (0..40)
        .map(|_| serde_json::json!({ "field": "code", "contains": "C" }))
        .collect();
    filters.push(serde_json::json!({ "field": "id", "from": "1", "to": "20" }));
    let call = serde_json::json!({ "screen": "item_detail", "params": { "id": 4 }, "node": 0, "query": { "page": { "type": "offset", "offset": 0, "limit": 50 }, "filters": filters } });
    let r = send(&router, post("/_zeyn/grid", call)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(
        r.json()["count"],
        1,
        "the scope stays whatever the client sends"
    );
    let router = master_router(reader());
    let r = send(
        &router,
        get("/_zeyn/screen/item_detail?params=%7B%22id%22%3A4%7D"),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[test]
fn blank_filters_keep_empty_or_set_values() {
    let rows: Vec<Item> = (1..=20).map(item).collect();
    let empty = DataQuery {
        filters: vec![zeyn::Filter::blank("due", true)],
        ..DataQuery::default()
    }
    .apply(rows.clone());
    let set = DataQuery {
        filters: vec![zeyn::Filter::blank("due", false)],
        ..DataQuery::default()
    }
    .apply(rows);
    assert_eq!((empty.total, set.total), (Some(15), Some(5)));
}

#[test]
fn apply_groups_rows_with_counts_totals_sort_and_paging() {
    let rows: Vec<Item> = (1..=20).map(item).collect();
    let mut q = query(
        serde_json::json!({ "page": { "type": "offset", "offset": 0, "limit": 2 }, "sort": [{ "field": "kind", "desc": true }], "aggregates": [{ "field": "qty", "op": "sum" }] }),
    );
    q.group = Some("kind".into());
    let page = q.apply(rows.clone());
    assert!(page.items.is_empty());
    assert_eq!(page.total, Some(3));
    let seen: Vec<(Value, u64)> = page
        .groups
        .iter()
        .map(|g| (g.value.clone(), g.count))
        .collect();
    assert_eq!(
        seen,
        [(Value::Choice("Tool"), 6), (Value::Choice("Part"), 7)]
    );
    assert_eq!(
        page.groups[0].totals[0].value,
        Value::Int(3 + 6 + 9 + 12 + 15 + 18)
    );
    assert!(page.next.is_some());
    let q = DataQuery {
        group: Some("due".into()),
        ..DataQuery::default()
    };
    let page = q.apply(rows);
    assert_eq!(
        (page.groups[0].value.clone(), page.groups[0].count),
        (Value::Null, 15),
        "empty values form one group, first"
    );
}

#[test]
fn any_of_matches_keys() {
    let rows: Vec<Item> = (1..=20).map(item).collect();
    let page = DataQuery {
        filters: vec![zeyn::Filter::any_of("id", ["3", "7", "99"])],
        ..DataQuery::default()
    }
    .apply(rows);
    assert_eq!(codes(&page), ["C-03", "C-07"]);
}

#[screen(route = "/grouped", permission = "items.read")]
pub fn grouped() -> Screen {
    Screen::new("Grouped").body(
        DataGrid::of(search)
            .paging(Paging::pages(2))
            .group_by(Item::kind)
            .group_by(Item::active)
            .column(Item::code)
            .column(Item::qty)
            .sum(),
    )
}

/// A node of a tree.
#[entity(table = "nodes")]
pub struct Node {
    #[key]
    pub id: NodeId,
    pub name: String,
    pub parent: Option<NodeId>,
}

#[action(permission = "items.read")]
pub async fn search_nodes(q: DataQuery) -> Result<Page<Node>> {
    let parent = |i: i64| match i {
        1 | 2 => None,
        3..=5 => Some(NodeId(1)),
        _ => Some(NodeId(3)),
    };
    Ok(q.apply(
        (1..=8)
            .map(|i| Node {
                id: NodeId(i),
                name: format!("N{i}"),
                parent: parent(i),
            })
            .collect(),
    ))
}

#[screen(route = "/tree", permission = "items.read")]
pub fn tree() -> Screen {
    Screen::new("Tree").body(
        DataGrid::of(search_nodes)
            .paging(Paging::pages(2))
            .tree(Node::parent)
            .column(Node::name),
    )
}

fn nested_router(identity: Identity) -> Router {
    App::new("Test")
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(
            Feature::new("items")
                .screen(grouped)
                .screen(tree),
        )
        .router()
        .unwrap()
}

fn branch(screen: &str, path: serde_json::Value) -> Request<Body> {
    post(
        "/_zeyn/grid",
        serde_json::json!({ "screen": screen, "node": 0, "path": path, "query": { "page": { "type": "offset", "offset": 0, "limit": 2 }, "with_total": true } }),
    )
}

#[tokio::test]
async fn grouped_grids_page_over_groups_and_open_them_level_by_level() {
    let router = nested_router(reader());
    let r = send(&router, get("/grouped")).await;
    assert!(
        r.body
            .contains(r#"<table role="treegrid""#)
    );
    assert!(r.body.contains(r#"<tr class="z-group" aria-level="1" data-zeyn-level="0" aria-expanded="false" data-zeyn-path="[&quot;Food&quot;]"><td colspan="1">"#), "{}", r.body);
    assert!(
        r.body.contains("7 rows")
            && r.body
                .contains(r#"data-zeyn-total="3""#)
    );
    let r = send(&router, branch("grouped", serde_json::json!(["Tool"]))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let rows = r.json()["rows"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        rows.contains(r#"data-zeyn-path="[&quot;Tool&quot;,&quot;false&quot;]""#)
            && rows.contains(r#"aria-level="2""#),
        "{rows}"
    );
    assert_eq!(r.json()["total"], 2);
    let r = send(
        &router,
        branch("grouped", serde_json::json!(["Tool", "true"])),
    )
    .await;
    let rows = r.json()["rows"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        rows.contains(r#"<tr aria-level="3" data-zeyn-level="2" data-zeyn-key="6">"#),
        "{rows}"
    );
    assert_eq!(r.json()["total"], 3, "tools 6, 12, 18 are active");
    assert_eq!(
        send(
            &router,
            branch("grouped", serde_json::json!(["Tool", "true", "x"]))
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    let long: Vec<&str> = vec!["a"; 33];
    assert_eq!(
        send(&router, branch("grouped", serde_json::json!(long)))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let router = nested_router(Identity::user("n"));
    assert_eq!(
        send(&router, branch("grouped", serde_json::json!(["Tool"])))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn tree_grids_show_top_rows_and_load_children_lazily() {
    let router = nested_router(reader());
    let r = send(&router, get("/tree")).await;
    assert!(r.body.contains(r#"<tr aria-level="1" data-zeyn-level="0" aria-expanded="false" data-zeyn-path="[&quot;1&quot;]" data-zeyn-key="1">"#), "{}", r.body);
    assert!(r.body.contains(r#"<tr aria-level="1" data-zeyn-level="0" data-zeyn-key="2"><td class="z-tree-cell"><span class="z-tree" style="--z-level:0"><span class="z-tree-spacer""#), "N2 has no children");
    let r = send(&router, branch("tree", serde_json::json!(["1"]))).await;
    let rows = r.json()["rows"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(r.json()["total"], 3);
    assert!(
        rows.contains(r#"data-zeyn-path="[&quot;1&quot;,&quot;3&quot;]" data-zeyn-key="3""#)
            && !rows.contains("N5"),
        "{rows}"
    );
    let r = send(&router, branch("tree", serde_json::json!(["1", "3"]))).await;
    assert_eq!(r.json()["total"], 3, "6, 7 and 8 report to 3");
    assert_eq!(
        send(&router, branch("tree", serde_json::json!([null])))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn pasted_rows_are_saved_one_by_one_and_each_is_checked_on_its_own() {
    let (router, store) = edit_router(writer_items());
    let pastes = [
        (
            "1",
            serde_json::json!({ "code": "P-1", "price": "1.234,50", "version": "1" }),
            StatusCode::OK,
        ),
        (
            "2",
            serde_json::json!({ "code": "", "version": "1" }),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "3",
            serde_json::json!({ "code": "P-3", "qty": "99", "version": "1" }),
            StatusCode::FORBIDDEN,
        ),
        (
            "4",
            serde_json::json!({ "code": "P-4", "version": "0" }),
            StatusCode::CONFLICT,
        ),
    ];
    for (key, values, status) in pastes {
        let r = send(
            &router,
            post_as(
                "/_zeyn/grid/edit",
                serde_json::json!({ "screen": "edit_list", "node": 0, "key": key, "values": values }),
                "tr",
            ),
        )
        .await;
        assert_eq!(r.status, status, "row {key}: {}", r.body);
    }
    let rows = store.0.read().unwrap().clone();
    assert_eq!(
        (rows[0].code.as_str(), rows[0].price),
        ("P-1", Money::tl(123_450)),
        "Turkish money as a spreadsheet copies it"
    );
    assert_eq!(
        (
            rows[1].code.as_str(),
            rows[2].code.as_str(),
            rows[2].qty,
            rows[3].code.as_str()
        ),
        ("C-02", "C-03", 3, "C-04")
    );
}

#[tokio::test]
async fn grid_writes_count_against_the_write_limit() {
    let store = Store::seeded();
    let identity = writer_items();
    let router = App::new("Test")
        .service(store)
        .write_limit(zeyn::RateLimit::per_user(
            2,
            std::time::Duration::from_secs(60),
        ))
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .feature(Feature::new("items").screen(edit_list))
        .router()
        .unwrap();
    let mut statuses = Vec::new();
    for i in 1..=3 {
        statuses.push(
            send(
                &router,
                edit(
                    &i.to_string(),
                    serde_json::json!({ "code": format!("W-{i}"), "version": "1" }),
                ),
            )
            .await
            .status,
        );
    }
    assert_eq!(
        statuses,
        [
            StatusCode::OK,
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS
        ]
    );
    let r = send(
        &router,
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "edit_list", "node": 0 }),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "reading rows is not a write");
}

/// Refuses every call in `before` and counts the calls `after` sees.
#[derive(Clone, Default)]
struct RefuseAll(Arc<RwLock<Vec<bool>>>);

impl zeyn::Interceptor for RefuseAll {
    fn before(&self, _ctx: &Ctx, _call: &zeyn::server::Call) -> Result<()> {
        Err(Error::Forbidden)
    }

    fn after(&self, _ctx: &Ctx, _call: &zeyn::server::Call, outcome: &zeyn::server::CallOutcome) {
        self.0.write().unwrap().push(outcome.ok);
    }
}

#[tokio::test]
async fn an_export_refused_by_an_interceptor_still_reaches_its_after() {
    let refuse = RefuseAll::default();
    let identity = reader();
    let router = App::new("Test")
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .interceptor(refuse.clone())
        .feature(Feature::new("items").screen(export_list))
        .router()
        .unwrap();
    let r = send(&router, export_get(0, serde_json::json!({}), "en")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        *refuse.0.read().unwrap(),
        [false],
        "audit interceptors record the refused export"
    );
}

#[screen(route = "/long", permission = "items.read")]
pub fn long_list() -> Screen {
    Screen::new("Long").body(
        DataGrid::of(search)
            .paging(Paging::virtual_scroll())
            .column(Item::code),
    )
}

#[tokio::test]
async fn virtual_grids_render_one_screen_of_rows_live_and_keep_the_rest_of_the_block_inert() {
    let rows = (1..=150)
        .map(|i| Item {
            id: ItemId(i),
            code: format!("R-{i:03}"),
            ..item(i % 28 + 1)
        })
        .collect();
    let router = App::new("Test")
        .service(Store(Arc::new(RwLock::new(rows))))
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(reader))
        .feature(Feature::new("items").screen(long_list))
        .router()
        .unwrap();
    let r = send(&router, get("/long")).await;
    let (live, later) = r
        .body
        .split_once("<template data-zeyn-rows-later>")
        .expect("rows after the first screen wait in a template");
    let later = &later[..later.find("</template>").unwrap()];
    assert_eq!(
        live.matches("R-").count(),
        zeyn::VIRTUAL_LIVE_ROWS,
        "{live}"
    );
    assert!(live.contains("R-064") && !live.contains("R-065"));
    assert_eq!(later.matches("R-").count(), 150 - zeyn::VIRTUAL_LIVE_ROWS);
    assert!(later.starts_with("<tr") && later.contains("R-150"));
}

#[screen(route = "/unsorted", permission = "items.read")]
pub fn unsorted() -> Screen {
    Screen::new("Unsorted").body(
        DataGrid::of(search)
            .column(Item::code)
            .column(Item::qty)
            .no_sort(),
    )
}

#[tokio::test]
async fn columns_without_sorting_get_no_sort_button_and_the_server_drops_their_sort_keys() {
    let router = App::new("Test")
        .service(Store::seeded())
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(reader))
        .feature(Feature::new("items").screen(unsorted))
        .router()
        .unwrap();
    let page = send(&router, get("/unsorted"))
        .await
        .body;
    assert!(
        page.contains(r#"data-field="code" aria-sort="none""#),
        "{page}"
    );
    assert!(
        page.contains(r#"data-field="qty" data-sort="off""#),
        "{page}"
    );
    let q = serde_json::json!({ "sort": [{ "field": "qty", "desc": true }] });
    let call = serde_json::json!({ "screen": "unsorted", "node": 0, "query": q });
    let rows = send(&router, post("/_zeyn/grid", call))
        .await
        .json()["rows"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        rows.find("C-01").unwrap() < rows.find("C-20").unwrap(),
        "the action never saw the sort: {rows}"
    );
}

/// Rows as `rows_html` renders them (the row renderer writes straight into its output; the
/// pinned action cell carries its pin class): every value kind, escaping, editable cells with
/// `data-v`, keys, versions, an open effect and
/// a row action.
const ROWS_BEFORE: &str = r##"<tr tabindex="-1" data-zeyn-effect="{&quot;effect&quot;:&quot;toast&quot;,&quot;message&quot;:&quot;open &lt;a href=\&quot;x\&quot;&gt;&amp;&#39;&lt;/a&gt;&quot;}" data-zeyn-key="3" data-zeyn-version="1"><td class="z-num">3</td><td data-v="&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;">&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;</td><td>Tool</td><td class="z-num" data-v="30.00">₺30.00</td><td class="z-num">3</td><td data-v="false">No</td><td><time datetime="2026-01-03">1/3/2026</time></td><td></td><td>note 3</td><td class="z-row-actions z-pin z-pe1 z-pin-edge-e"><button type="button" class="z-row-action" data-zeyn-run="a0" tabindex="-1"><span>Bump</span></button></td></tr><tr tabindex="-1" data-zeyn-effect="{&quot;effect&quot;:&quot;toast&quot;,&quot;message&quot;:&quot;open C-04&quot;}" data-zeyn-key="4" data-zeyn-version="1"><td class="z-num">4</td><td data-v="C-04">C-04</td><td>Part</td><td class="z-num" data-v="40.00">₺40.00</td><td class="z-num">4</td><td data-v="true">Yes</td><td><time datetime="2026-01-04">1/4/2026</time></td><td><time datetime="2026-03-04">3/4/2026</time></td><td>ünïcode ✓ &amp; &quot;quoted&quot;</td><td class="z-row-actions z-pin z-pe1 z-pin-edge-e"><button type="button" class="z-row-action" data-zeyn-run="a0" tabindex="-1"><span>Bump</span></button></td></tr><tr tabindex="-1" data-zeyn-effect="{&quot;effect&quot;:&quot;toast&quot;,&quot;message&quot;:&quot;open C-05&quot;}" data-zeyn-key="5" data-zeyn-version="1"><td class="z-num">5</td><td data-v="C-05">C-05</td><td>Food</td><td class="z-num" data-v="-12,345.67">-₺12,345.67</td><td class="z-num">5</td><td data-v="false">No</td><td><time datetime="2026-01-05">1/5/2026</time></td><td></td><td>note 5</td><td class="z-row-actions z-pin z-pe1 z-pin-edge-e"><button type="button" class="z-row-action" data-zeyn-run="a0" tabindex="-1"><span>Bump</span></button></td></tr>"##;

#[test]
fn row_html_stays_byte_for_byte_the_same() {
    let grid = DataGrid::of(search)
        .column(Item::id)
        .column(Item::code)
        .editable()
        .column(Item::kind)
        .column(Item::price)
        .editable()
        .column(Item::qty)
        .column(Item::active)
        .editable()
        .column(Item::added)
        .column(Item::due)
        .column(Item::note)
        .editable()
        .on_open(|i| Effect::Toast {
            message: Text::from(format!("open {}", i.code)),
        })
        .row_action("Bump", bump, |i| i.id)
        .edit_with(save_item);
    let mut rows: Vec<Item> = (3..=5).map(item).collect();
    rows[0].code = r#"<a href="x">&'</a>"#.into();
    rows[1].note = "ünïcode ✓ & \"quoted\"".into();
    rows[2].price = Money::tl(-1_234_567);
    assert_eq!(grid.rows_html(&rows), ROWS_BEFORE);
}

#[screen(route = "/wide", permission = "items.read")]
pub fn wide_list() -> Screen {
    let mut grid = DataGrid::of(search)
        .paging(Paging::virtual_scroll())
        .column(Item::code)
        .done();
    for _ in 1..zeyn::VIRTUAL_WIDE_COLUMNS {
        grid = grid.column(Item::qty).done();
    }
    Screen::new("Wide").body(grid)
}

#[tokio::test]
async fn wide_virtual_grids_keep_every_row_of_the_first_block_in_the_template() {
    let rows = (1..=150)
        .map(|i| Item {
            id: ItemId(i),
            code: format!("R-{i:03}"),
            ..item(i % 28 + 1)
        })
        .collect();
    let router = App::new("Test")
        .service(Store(Arc::new(RwLock::new(rows))))
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(reader))
        .feature(
            Feature::new("items")
                .screen(wide_list)
                .screen(long_list),
        )
        .router()
        .unwrap();
    let r = send(&router, get("/wide")).await;
    let (live, later) = r
        .body
        .split_once("<template data-zeyn-rows-later>")
        .expect("the block waits in a template");
    let later = &later[..later.find("</template>").unwrap()];
    let body = &live[live
        .find("<tbody data-zeyn-rows>")
        .unwrap()..];
    assert!(
        body.starts_with("<tbody data-zeyn-rows></tbody>"),
        "no live rows: {}",
        &body[..80]
    );
    assert_eq!(later.matches("R-").count(), 150);
    assert!(later.starts_with("<tr") && later.contains("R-001") && later.contains("R-150"));

    let narrow = send(&router, get("/long")).await.body;
    assert!(
        narrow.contains("R-001</td>") && !narrow.contains("<tbody data-zeyn-rows></tbody>"),
        "narrow grids keep live rows"
    );
}

#[test]
fn row_and_bulk_actions_can_ask_for_a_confirmation_and_look_destructive() {
    let grid = DataGrid::of(search)
        .column(Item::code)
        .row_action("Bump", bump, |i| i.id)
        .row_action("Bump <again>", bump, |i| i.id)
        .confirm("Bump <this> row?")
        .tone(zeyn::Tone::Danger)
        .bulk_action("Deactivate", deactivate, Item::id)
        .confirm("Deactivate the selected rows?")
        .tone(zeyn::Tone::Danger)
        .bulk_action("Deactivate quietly", deactivate, Item::id);
    let rows = Store::seeded()
        .0
        .read()
        .unwrap()
        .clone();
    let html = grid.rows_html(&rows[..1]);
    assert!(
        html.contains(
            r#"<button type="button" class="z-row-action" data-zeyn-run="a0" tabindex="-1">"#
        ),
        "{html}"
    );
    assert!(
        html.contains(r#"<button type="button" class="z-row-action z-danger" data-zeyn-run="a1" data-zeyn-confirm="Bump &lt;this&gt; row?" tabindex="-1">"#),
        "{html}"
    );
    let mut page = String::new();
    zeyn::Component::render(&grid, &mut page, &zeyn::Render::english("s"));
    assert!(page.contains(r#"class="z-button z-danger-button" data-zeyn-bulk-run="0" data-testid="s.grid.bulk-0" data-zeyn-confirm="Deactivate the selected rows?">Deactivate</button>"#), "{page}");
    assert!(page.contains(r#"class="z-button" data-zeyn-bulk-run="1" data-testid="s.grid.bulk-1">Deactivate quietly</button>"#));
    assert!(
        page.contains("&quot;confirm_title&quot;:&quot;Please confirm&quot;"),
        "the dialog texts come with the grid"
    );
}

#[test]
fn whole_numbers_are_grouped_by_locale_right_aligned_and_plain_on_request() {
    let mut row = Store::seeded().0.read().unwrap()[0].clone();
    row.id = ItemId(1_000_002);
    row.qty = -1_234_567;
    let by_locale = DataGrid::of(search)
        .column(Item::id)
        .column(Item::qty);
    let en = by_locale
        .done()
        .rows_html(std::slice::from_ref(&row));
    assert!(
        en.contains(r#"<td class="z-num">1000002</td>"#),
        "keys stay plain: {en}"
    );
    assert!(en.contains(r#"<td class="z-num">-1,234,567</td>"#), "{en}");
    let i18n = I18n::builtin();
    let mut tr = String::new();
    zeyn::Render::new("s", "tr", i18n).value_into(
        &mut tr,
        &Value::Int(1_234_567),
        &zeyn::FieldKind::Integer,
    );
    assert_eq!(tr, "1.234.567");
    let mut fr = String::new();
    zeyn::Render::new("s", "fr", i18n).value_into(
        &mut fr,
        &Value::Int(12_345),
        &zeyn::FieldKind::Integer,
    );
    assert_eq!(fr, "12\u{202f}345");
    let plain = DataGrid::of(search)
        .column(Item::qty)
        .plain()
        .done()
        .rows_html(std::slice::from_ref(&row));
    assert!(
        plain.contains(r#"<td class="z-num">-1234567</td>"#),
        "{plain}"
    );
    let mut small = String::new();
    zeyn::render::push_grouped(&mut small, 999, "en");
    zeyn::render::push_grouped(&mut small, i64::MIN, "en");
    assert_eq!(small, "999-9,223,372,036,854,775,808");
}

/// A million computed rows; only the asked page is ever built.
#[action(permission = "items.read")]
pub async fn million(q: DataQuery) -> Result<Page<Item>> {
    const TOTAL: u64 = 1_000_000;
    let span = q.span();
    let rows = (span.offset..span.end().min(TOTAL))
        .map(|n| {
            let mut row = item((n % 20 + 1) as i64);
            row.id = ItemId(n as i64 + 1);
            row
        })
        .collect();
    Ok(Page::window(rows, TOTAL, span))
}

#[test]
fn computed_tables_page_through_span_and_window_with_either_page_form() {
    let query = |page: serde_json::Value| -> DataQuery {
        serde_json::from_value(serde_json::json!({ "page": page })).unwrap()
    };
    let numbered = query(serde_json::json!({ "type": "offset", "offset": 999_990, "limit": 50 }));
    assert_eq!(
        numbered.span(),
        zeyn::Span {
            offset: 999_990,
            limit: 50
        }
    );
    let after = query(serde_json::json!({ "type": "after", "cursor": "120", "limit": 100_000 }));
    assert_eq!(
        after.span(),
        zeyn::Span {
            offset: 120,
            limit: zeyn::MAX_PAGE_SIZE
        },
        "the client cannot ask for more than the cap"
    );
    let odd = query(serde_json::json!({ "type": "after", "cursor": "x", "limit": 10 }));
    assert_eq!(odd.span().offset, 0);

    let last = Page::window(
        vec![1, 2, 3],
        10,
        zeyn::Span {
            offset: 7,
            limit: 5,
        },
    );
    assert_eq!((last.total, last.next), (Some(10), None));
    let middle = Page::window(
        vec![1, 2],
        10,
        zeyn::Span {
            offset: 2,
            limit: 2,
        },
    );
    assert_eq!(middle.next.unwrap().0, "4");
    let past = Page::<i32>::window(
        Vec::new(),
        10,
        zeyn::Span {
            offset: 50,
            limit: 5,
        },
    );
    assert!(
        past.next.is_none() && past.items.is_empty(),
        "no cursor that loops"
    );
    let all = [1, 2, 3, 4, 5];
    let slice = Page::from_slice(
        &all,
        &query(serde_json::json!({ "type": "offset", "offset": 3, "limit": 2 })),
    );
    assert_eq!((slice.items, slice.next), (vec![4, 5], None));
}

#[tokio::test]
async fn a_grid_over_a_computed_million_rows_loads_only_its_page() {
    let page = million::run(
        serde_json::from_value(
            serde_json::json!({ "page": { "type": "offset", "offset": 750_000, "limit": 25 } }),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 25);
    assert_eq!(page.items[0].id, ItemId(750_001));
    assert_eq!(page.total, Some(1_000_000));
    assert_eq!(page.next.unwrap().0, "750025");
}

/// Person with a nullable name, for comparing sort orders with ties, nulls and Turkish letters.
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: Option<String>,
    pub city: String,
    pub score: i64,
}

/// The order of `DataQuery::apply` before it read each sort value once: a stable sort that reads
/// both values for every comparison, text by lower case then as written, nulls first, ties by
/// the key.
fn reference_order(rows: &[Person], sort: &[(usize, bool)]) -> Vec<i64> {
    fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (a, b) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            (Value::Text(x), Value::Text(y)) => x
                .to_lowercase()
                .cmp(&y.to_lowercase())
                .then_with(|| x.cmp(y)),
            _ => Ordering::Equal,
        }
    }
    let mut order = sort.to_vec();
    order.push((0, false));
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| {
        for (i, desc) in &order {
            let o = compare(
                &a.value_at(*i).unwrap_or(Value::Null),
                &b.value_at(*i).unwrap_or(Value::Null),
            );
            let o = if *desc { o.reverse() } else { o };
            if o.is_ne() {
                return o;
            }
        }
        std::cmp::Ordering::Equal
    });
    rows.iter().map(|p| p.id.0).collect()
}

#[test]
fn sorted_pages_match_a_full_stable_sort_with_ties_nulls_and_turkish_text() {
    let names = [
        Some("ılık"),
        Some("Ilık"),
        Some("İpek"),
        Some("ipek"),
        Some("Çınar"),
        Some("çınar"),
        Some("Zeynep"),
        Some("ÖMER"),
        Some("ömer"),
        None,
        Some(""),
        Some("ǅemal"),
    ];
    let cities = ["İzmir", "izmir", "Ankara", "ankara", "Şile"];
    let people: Vec<Person> = (1..=500i64)
        .rev()
        .map(|i| {
            let h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33;
            Person {
                id: PersonId(i),
                name: names[(h % names.len() as u64) as usize].map(String::from),
                city: cities[(h / 16 % cities.len() as u64) as usize].to_string(),
                score: (h % 7) as i64,
            }
        })
        .collect();
    let field = |i: usize| Person::fields()[i].name.to_string();
    for sort in [
        vec![(1, false)],
        vec![(1, true)],
        vec![(2, false), (1, true)],
        vec![(3, true), (2, false)],
        vec![],
    ] {
        let all = reference_order(&people, &sort);
        for (offset, limit) in [(0, 25), (0, 500), (230, 40), (480, 50), (600, 10)] {
            let q = DataQuery {
                page: zeyn::PageRequest::Offset { offset, limit },
                sort: sort
                    .iter()
                    .map(|(i, desc)| zeyn::Sort {
                        field: field(*i),
                        desc: *desc,
                    })
                    .collect(),
                ..DataQuery::default()
            };
            let page = q.apply(people.clone());
            let got: Vec<i64> = page
                .items
                .iter()
                .map(|p| p.id.0)
                .collect();
            let start = (offset as usize).min(all.len());
            let end = (start + limit as usize).min(all.len());
            assert_eq!(got, all[start..end], "sort {sort:?}, offset {offset}");
            assert_eq!(page.total, Some(500));
            assert_eq!(page.next.is_some(), end < all.len());
        }
        let after = DataQuery {
            page: zeyn::PageRequest::After {
                cursor: Some(zeyn::Cursor("100".into())),
                limit: 30,
            },
            sort: sort
                .iter()
                .map(|(i, desc)| zeyn::Sort {
                    field: field(*i),
                    desc: *desc,
                })
                .collect(),
            with_total: true,
            ..DataQuery::default()
        };
        let got: Vec<i64> = after
            .apply(people.clone())
            .items
            .iter()
            .map(|p| p.id.0)
            .collect();
        assert_eq!(got, all[100..130], "after cursor, sort {sort:?}");
    }
}

/// More rows than `DataQuery::apply` sorts in place on a tokio worker (50,000).
fn many_people() -> Vec<Person> {
    (1..=50_010i64)
        .map(|i| Person {
            id: PersonId(i),
            name: Some(format!("P{}", i % 977)),
            city: String::new(),
            score: i % 13,
        })
        .collect()
}

fn by_name_desc() -> DataQuery {
    DataQuery {
        sort: vec![zeyn::Sort {
            field: "name".into(),
            desc: true,
        }],
        ..DataQuery::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_lists_sort_off_the_worker_on_the_multi_threaded_runtime() {
    let page = by_name_desc().apply(many_people());
    assert_eq!(page.total, Some(50_010));
    assert_eq!(page.items[0].name.as_deref(), Some("P99"));
    assert_eq!(page.items[0].id.0, 99);
}

#[tokio::test]
async fn large_lists_sort_in_place_on_a_current_thread_runtime() {
    let page = by_name_desc().apply(many_people());
    assert_eq!(page.items[0].name.as_deref(), Some("P99"));
}

#[test]
fn groups_keep_their_order_and_totals_after_the_cached_sort() {
    let people: Vec<Person> = (1..=40i64)
        .map(|i| Person {
            id: PersonId(i),
            name: None,
            city: ["İzmir", "Ankara", "izmir", "Şile"][(i % 4) as usize].to_string(),
            score: i,
        })
        .collect();
    let q = DataQuery {
        group: Some("city".into()),
        aggregates: vec![AggregateRequest {
            field: "score".into(),
            op: Aggregate::Sum,
        }],
        ..DataQuery::default()
    };
    let page = q.apply(people);
    let cities: Vec<String> = page
        .groups
        .iter()
        .map(|g| match &g.value {
            Value::Text(t) => t.clone(),
            v => format!("{v:?}"),
        })
        .collect();
    assert_eq!(cities, ["Ankara", "izmir", "İzmir", "Şile"]);
    assert!(page.items.is_empty());
    assert_eq!(
        page.groups
            .iter()
            .map(|g| g.count)
            .sum::<u64>(),
        40
    );
    assert_eq!(page.totals[0].value, Value::Int(820));
}

/// The last query the wide dynamic action got, to see what reaches the action.
static DYNAMIC_QUERY: std::sync::Mutex<Option<DataQuery>> = std::sync::Mutex::new(None);

#[action(permission = "sql.run")]
pub async fn preview_sql(_q: DataQuery) -> Result<DynamicPage> {
    use zeyn::{DynamicColumn as C, DynamicKind as K};
    let columns = vec![
        C::new("name", "Name", K::Text),
        C::new("qty", "Quantity", K::Integer),
        C::new("rate", "Rate", K::Decimal),
        C::new("on", "Active", K::Bool),
        C::new("day", "Day", K::Date),
        C::new("at", "At", K::DateTime),
    ];
    let rows = vec![
        serde_json::json!([
            "Ada",
            1234567,
            "1234.5",
            true,
            "2026-03-31",
            "2026-03-31T11:30:00Z"
        ]),
        serde_json::json!({ "name": "Can", "qty": "42", "rate": 0.25, "on": 0, "day": "not a date" }),
    ];
    Ok(DynamicPage::new(columns, rows).total(2))
}

#[action(permission = "sql.run")]
pub async fn hostile_sql(_q: DataQuery) -> Result<DynamicPage> {
    let columns = vec![zeyn::DynamicColumn::new(
        r#""><script>alert(1)</script>"#,
        r#"<img src=x onerror="alert(2)">'"#,
        zeyn::DynamicKind::Text,
    )];
    let rows = vec![serde_json::json!([
        r#"<script>alert(3)</script><a href="javascript:x" onclick='y'>"#
    ])];
    Ok(DynamicPage::new(columns, rows))
}

#[action(permission = "sql.run")]
pub async fn wide_sql(q: DataQuery) -> Result<DynamicPage> {
    *DYNAMIC_QUERY.lock().unwrap() = Some(q);
    let columns = (0..300)
        .map(|i| {
            zeyn::DynamicColumn::new(
                format!("c{i}{}", "x".repeat(if i == 0 { 500 } else { 0 })),
                format!("C{i}"),
                zeyn::DynamicKind::Text,
            )
        })
        .collect();
    let rows = (0..2_000)
        .map(|_| serde_json::json!(["y".repeat(5_000)]))
        .collect();
    Ok(DynamicPage::new(columns, rows).total(2_000))
}

#[screen(route = "/sql", permission = "items.read")]
pub fn sql() -> Screen {
    Screen::new("SQL").body(
        Stack::new()
            .add(DataGrid::dynamic(preview_sql).paging(Paging::pages(10)))
            .add(DataGrid::dynamic(hostile_sql)),
    )
}

/// Only the caps test opens this screen, so the query the action records is its own.
#[screen(route = "/sql-wide", permission = "items.read")]
pub fn sql_wide() -> Screen {
    Screen::new("SQL").body(DataGrid::dynamic(wide_sql).paging(Paging::virtual_scroll()))
}

fn dynamic_app(identity: Identity, audit: MemoryAudit) -> App {
    App::new("Test")
        .i18n(
            I18n::new("en")
                .catalog("tr", "{}")
                .unwrap(),
        )
        .auth(move |h: &HeaderMap| {
            h.contains_key("x-user")
                .then(|| identity.clone())
        })
        .interceptor(AuditLog(audit))
        .mcp(Mcp::on())
        .feature(
            Feature::new("sql")
                .screen(sql)
                .screen(sql_wide)
                .screen(sql_odd),
        )
}

fn dynamic_router(identity: Identity) -> Router {
    dynamic_app(identity, MemoryAudit::default())
        .router()
        .unwrap()
}

fn sql_user() -> Identity {
    Identity::user("ada").with_permissions(["items.read", "sql.run"])
}

fn dynamic_call(node: &str, query: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": "sql", "node": node, "query": query })
}

#[tokio::test]
async fn dynamic_grid_renders_the_columns_of_its_page_formatted_by_kind() {
    let router = dynamic_router(sql_user());
    let page = send(&router, get("/sql")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body
            .contains(r#"data-zeyn-shape=""#),
        "{}",
        page.body
    );
    assert!(page.body.contains(">Quantity</th>"), "{}", page.body);
    let r = send(
        &router,
        post_as(
            "/_zeyn/grid",
            dynamic_call("0.0", serde_json::json!({})),
            "tr",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    let head = j["head"].as_str().unwrap();
    assert!(
        head.contains(r#"class="z-num" data-field="qty" data-sort="off""#),
        "{head}"
    );
    assert_eq!(j["columns"][2]["label"], "Rate");
    let rows = j["rows"].as_str().unwrap();
    assert!(
        rows.contains(r#"<td class="z-num">1.234.567</td>"#),
        "{rows}"
    );
    assert!(rows.contains("1.234,5"), "{rows}");
    assert!(rows.contains(r#"<time datetime="2026-03-31">"#), "{rows}");
    assert!(rows.contains(r#"<td class="z-num">42</td>"#), "{rows}");
    assert!(rows.contains("not a date"), "{rows}");
    assert_eq!(j["count"], 2);
    let en = send(
        &router,
        post("/_zeyn/grid", dynamic_call("0.0", serde_json::json!({}))),
    )
    .await
    .json();
    assert!(
        en["rows"]
            .as_str()
            .unwrap()
            .contains("1,234,567")
    );
}

#[tokio::test]
async fn dynamic_grid_sends_its_header_only_when_the_columns_change() {
    let router = dynamic_router(sql_user());
    let first = send(
        &router,
        post("/_zeyn/grid", dynamic_call("0.0", serde_json::json!({}))),
    )
    .await
    .json();
    let shape = first["shape"]
        .as_str()
        .unwrap()
        .to_string();
    let mut call = dynamic_call("0.0", serde_json::json!({}));
    call["shape"] = shape.clone().into();
    let same = send(&router, post("/_zeyn/grid", call.clone()))
        .await
        .json();
    assert!(same["head"].is_null(), "{same}");
    assert!(
        same["rows"]
            .as_str()
            .unwrap()
            .contains("Ada")
    );
    call["head"] = true.into();
    let asked = send(&router, post("/_zeyn/grid", call))
        .await
        .json();
    assert_eq!(asked["shape"], shape.as_str());
    assert!(asked["head"].is_string());
}

#[tokio::test]
async fn dynamic_grid_escapes_hostile_column_names_labels_and_cells() {
    let router = dynamic_router(sql_user());
    let page = send(&router, get("/sql")).await.body;
    let j = send(
        &router,
        post("/_zeyn/grid", dynamic_call("0.1", serde_json::json!({}))),
    )
    .await
    .json();
    let head = j["head"].as_str().unwrap();
    let rows = j["rows"].as_str().unwrap();
    for html in [page.as_str(), head, rows] {
        assert!(!html.contains("<script>alert"), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(!html.contains("<a href"), "{html}");
        assert!(!html.contains(r#"onerror="alert"#), "{html}");
        assert!(!html.contains(r#"data-field=""><"#), "{html}");
    }
    assert!(
        head.contains("&lt;img src=x onerror=&quot;alert(2)&quot;&gt;"),
        "{head}"
    );
    assert!(rows.contains("&lt;script&gt;alert(3)"), "{rows}");
    assert_eq!(j["columns"][0]["field"], r#""><script>alert(1)</script>"#);
}

#[tokio::test]
async fn dynamic_grid_caps_rows_columns_names_and_text_from_the_action() {
    let router = dynamic_router(sql_user());
    let j = send(
        &router,
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "sql_wide", "node": "0", "query": {
                    "page": { "type": "offset", "offset": 0, "limit": 100_000 },
                    "sort": [{ "field": "c1", "desc": true }],
                    "filters": [{ "field": "c1", "contains": "y" }],
                    "aggregates": [{ "field": "c1", "op": "count" }]
            } }),
        ),
    )
    .await
    .json();
    let q = DYNAMIC_QUERY
        .lock()
        .unwrap()
        .take()
        .unwrap();
    assert_eq!(q.limit(), zeyn::MAX_PAGE_SIZE);
    assert!(q.sort.is_empty() && q.filters.is_empty() && q.aggregates.is_empty());
    assert_eq!(j["count"], zeyn::MAX_PAGE_CELLS / zeyn::MAX_DYNAMIC_COLUMNS);
    assert_eq!(
        j["next"],
        (zeyn::MAX_PAGE_CELLS / zeyn::MAX_DYNAMIC_COLUMNS).to_string()
    );
    let columns = j["columns"].as_array().unwrap();
    assert_eq!(columns.len(), zeyn::MAX_DYNAMIC_COLUMNS);
    assert_eq!(
        columns[0]["field"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        zeyn::MAX_COLUMN_NAME + 1
    );
    let rows = j["rows"].as_str().unwrap();
    let first = rows.split("</td>").next().unwrap();
    assert_eq!(
        first.matches('y').count(),
        zeyn::MAX_PAGE_TEXT / (500 * zeyn::MAX_DYNAMIC_COLUMNS),
        "each cell gets its share of the page's text budget: {first}"
    );
    assert!(first.ends_with('…'));
}

#[tokio::test]
async fn dynamic_grid_survives_the_largest_offsets_and_cursors() {
    let router = dynamic_router(sql_user());
    for page in [
        serde_json::json!({ "type": "offset", "offset": u64::MAX, "limit": 100_000 }),
        serde_json::json!({ "type": "offset", "offset": i64::MAX, "limit": u32::MAX }),
        serde_json::json!({ "type": "after", "cursor": u64::MAX.to_string(), "limit": 100_000 }),
    ] {
        let res = router
            .clone()
            .oneshot(post(
                "/_zeyn/grid",
                serde_json::json!({ "screen": "sql_wide", "node": "0", "query": { "page": page } }),
            ))
            .await
            .unwrap();
        assert!(
            res.status() == StatusCode::OK || res.status() == StatusCode::BAD_REQUEST,
            "{page}: {}",
            res.status()
        );
    }
}

#[tokio::test]
async fn dynamic_grid_needs_its_actions_permission() {
    let router = dynamic_router(Identity::user("x").with_permissions(["items.read"]));
    let r = send(
        &router,
        post("/_zeyn/grid", dynamic_call("0.0", serde_json::json!({}))),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let page = send(&router, get("/sql")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(!page.body.contains("Ada"));
    assert!(!page.body.contains(">Quantity</th>"));
}

#[action(permission = "sql.run")]
pub async fn oversized_sql(_q: DataQuery) -> Result<DynamicPage> {
    let columns = (0..200)
        .map(|i| {
            zeyn::DynamicColumn::new(format!("c{i}"), format!("C{i}"), zeyn::DynamicKind::Text)
        })
        .collect();
    let nested = serde_json::Value::Array(
        (0..200_000)
            .map(|i| serde_json::json!({ "n": i }))
            .collect(),
    );
    let rows = (0..260)
        .map(|r| {
            let mut cells: Vec<serde_json::Value> = (0..200)
                .map(|_| serde_json::Value::String("😀".repeat(100)))
                .collect();
            if r == 0 {
                cells[1] = nested.clone();
            }
            serde_json::Value::Array(cells)
        })
        .collect();
    Ok(DynamicPage::new(columns, rows).total(260))
}

#[action(permission = "sql.run")]
pub async fn odd_names_sql(_q: DataQuery) -> Result<DynamicPage> {
    use zeyn::{DynamicColumn as C, DynamicKind as K};
    let columns = vec![
        C::new("id", "Id", K::Integer),
        C::new("id", "Id again", K::Integer),
        C::new(" ", "Blank", K::Text),
        C::new("card", "Card", K::Text).masked(zeyn::MaskStyle::LastFour),
    ];
    let rows = vec![
        serde_json::json!([1, 2, "array", "4111 1111 1111 1234"]),
        serde_json::json!({ "id": 7, " ": "object", "card": "5500 0000 0000 9876" }),
    ];
    Ok(DynamicPage::new(columns, rows).total(2))
}

/// Grids with odd results: a page past the budgets, and odd names with a masked column.
#[screen(route = "/sql-odd", permission = "items.read")]
pub fn sql_odd() -> Screen {
    Screen::new("SQL").body(
        Stack::new()
            .add(DataGrid::dynamic(oversized_sql).paging(Paging::pages(500)))
            .add(DataGrid::dynamic(odd_names_sql).sensitive("Payment", "id")),
    )
}

#[tokio::test]
async fn dynamic_grid_keeps_a_page_within_its_cell_and_text_budget() {
    let router = dynamic_router(sql_user());
    let call = serde_json::json!({ "screen": "sql_odd", "node": "0.0", "query": { "page": { "type": "offset", "offset": 0, "limit": 500 } } });
    let r = send(&router, post("/_zeyn/grid", call)).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    let rows = zeyn::MAX_PAGE_CELLS / 200;
    assert_eq!(j["count"], rows);
    assert_eq!(j["next"], rows.to_string(), "paging goes on after the cut");
    let html = j["rows"].as_str().unwrap();
    let text: usize = html
        .split("<td>")
        .skip(1)
        .map(|c| c.split("</td>").next().unwrap().len())
        .sum();
    assert!(text <= zeyn::MAX_PAGE_TEXT, "{text} bytes of cell text");
    assert!(
        html.len() < 2 * zeyn::MAX_PAGE_TEXT,
        "{} bytes of rows",
        html.len()
    );
    let nested = html
        .split("<td>")
        .nth(2)
        .unwrap()
        .split("</td>")
        .next()
        .unwrap();
    assert!(nested.starts_with("[{&quot;n&quot;:0}"), "{nested}");
    assert!(nested.ends_with('…') && nested.len() < 1_000, "{nested}");
    let emoji = html
        .split("<td>")
        .nth(3)
        .unwrap()
        .split("</td>")
        .next()
        .unwrap();
    assert!(emoji.len() <= 100, "{} bytes", emoji.len());
}

#[tokio::test]
async fn dynamic_grid_makes_empty_and_repeated_column_names_unique() {
    let router = dynamic_router(sql_user());
    let call = serde_json::json!({ "screen": "sql_odd", "node": "0.1", "query": {} });
    let j = send(&router, post("/_zeyn/grid", call))
        .await
        .json();
    let names: Vec<&str> = j["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["field"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["id", "id_2", "col_2", "card"]);
    let rows = j["rows"].as_str().unwrap();
    assert!(
        rows.contains(r#"<td class="z-num">1</td><td class="z-num">2</td><td>array</td>"#),
        "{rows}"
    );
    assert!(
        rows.contains(r#"<td class="z-num">7</td><td class="z-num">7</td><td>object</td>"#),
        "{rows}"
    );
}

#[tokio::test]
async fn masked_dynamic_columns_never_reach_html_or_agents_and_sensitive_reads_are_audited() {
    let audit = MemoryAudit::default();
    let router = dynamic_app(sql_user(), audit.clone())
        .router()
        .unwrap();
    let page = send(&router, get("/sql-odd")).await;
    let call = serde_json::json!({ "screen": "sql_odd", "node": "0.1", "query": {} });
    let grid = send(&router, post("/_zeyn/grid", call)).await;
    let tools = send(
        &router,
        mcp(serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })),
    )
    .await
    .json();
    let tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .find(|n| n == "sql_odd_0-1_search")
        .unwrap_or_else(|| panic!("the grid is an agent tool: {tools}"));
    let agent = send(
        &router,
        mcp(serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": { "page": { "type": "offset", "offset": 0, "limit": 10 } } } })),
    )
    .await;
    assert_eq!(agent.json()["result"]["isError"], false, "{}", agent.body);
    for body in [&page.body, &grid.body, &agent.body] {
        assert!(body.contains("•••• 1234"), "{body}");
        assert!(!body.contains("4111"), "{body}");
        assert!(!body.contains("5500"), "{body}");
    }
    let reads: Vec<Option<String>> = audit
        .entries()
        .iter()
        .flat_map(|e| e.reads.iter())
        .filter(|r| r.model == "Payment")
        .map(|r| r.id.clone())
        .collect();
    assert!(reads.contains(&Some("1".into())), "{reads:?}");
    assert!(reads.contains(&Some("7".into())), "{reads:?}");
}

fn mcp(body: serde_json::Value) -> Request<Body> {
    Request::post("/_zeyn/mcp")
        .header("x-user", "1")
        .header(header::AUTHORIZATION, "Bearer test")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn a_dynamic_grid_uses_a_view_only_for_the_columns_it_was_made_for() {
    let router = dynamic_router(sql_user());
    let first = send(
        &router,
        post("/_zeyn/grid", dynamic_call("0.0", serde_json::json!({}))),
    )
    .await
    .json();
    let shape = first["shape"].as_str().unwrap();
    let head = |view: serde_json::Value| {
        let mut call = dynamic_call("0.0", serde_json::json!({}));
        call["head"] = true.into();
        call["view"] = view;
        let router = router.clone();
        async move {
            send(&router, post("/_zeyn/grid", call))
                .await
                .json()["head"]
                .as_str()
                .unwrap()
                .to_string()
        }
    };
    let same = head(serde_json::json!({ "columns": [{ "field": "qty" }], "shape": shape })).await;
    assert!(
        same.contains(r#"data-field="qty""#) && !same.contains(r#"data-field="name""#),
        "{same}"
    );
    let other =
        head(serde_json::json!({ "columns": [{ "field": "qty" }], "shape": "0123456789abcdef" }))
            .await;
    assert!(other.contains(r#"data-field="name""#), "{other}");
    let none = head(serde_json::json!({ "columns": [{ "field": "qty" }] })).await;
    assert!(none.contains(r#"data-field="name""#), "{none}");
}

#[tokio::test]
async fn a_request_with_an_old_shape_gets_the_new_header() {
    let router = dynamic_router(sql_user());
    let mut call = dynamic_call("0.0", serde_json::json!({}));
    call["shape"] = "0123456789abcdef".into();
    let j = send(&router, post("/_zeyn/grid", call))
        .await
        .json();
    assert!(
        j["head"]
            .as_str()
            .unwrap()
            .contains(">Quantity</th>")
    );
    assert_ne!(j["shape"], "0123456789abcdef");
    assert_eq!(j["columns"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn typed_grid_view_naming_no_known_column_falls_back_to_the_grid_layout() {
    let body = serde_json::json!({
        "screen": "list", "node": 0, "head": true,
        "view": { "columns": [{ "field": "nope" }, { "field": "gone" }] }
    });
    let j = send(&router(reader()), post("/_zeyn/grid", body))
        .await
        .json();
    let head = j["head"].as_str().unwrap();
    assert!(head.contains(r#"data-field="code""#), "{head}");
    assert!(head.contains(r#"data-field="active""#), "{head}");
}

#[tokio::test]
async fn dynamic_grids_offer_no_export_edit_or_row_items() {
    let router = dynamic_router(sql_user());
    let export = send(&router, get("/_zeyn/grid/export?screen=sql&node=0.0")).await;
    assert_eq!(export.status, StatusCode::NOT_FOUND, "{}", export.body);
    let edit = send(
        &router,
        post(
            "/_zeyn/grid/edit",
            serde_json::json!({ "screen": "sql", "node": "0.0", "key": "1", "values": { "name": "x" } }),
        ),
    )
    .await;
    assert_eq!(edit.status, StatusCode::NOT_FOUND, "{}", edit.body);
    let run = send(
        &router,
        post(
            "/_zeyn/grid/run",
            serde_json::json!({ "screen": "sql", "node": "0.0", "key": "1", "item": "a0" }),
        ),
    )
    .await;
    assert_eq!(run.status, StatusCode::NOT_FOUND, "{}", run.body);
}

#[screen(route = "/banded", permission = "items.read")]
pub fn banded() -> Screen {
    Screen::new("Banded").body(
        DataGrid::of(search)
            .search()
            .selectable()
            .exportable()
            .column(Item::code)
            .pin_start()
            .band("Stock", |b| {
                b.column(Item::price)
                    .column(Item::qty)
                    .column(Item::active)
                    .hidden()
            })
            .column(Item::added),
    )
}

fn banded_call(query: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": "banded", "node": 0, "query": query })
}

#[tokio::test]
async fn search_box_sends_q_and_the_action_searches_text_fields() {
    let router = router(reader());
    let page = send(&router, get("/banded")).await;
    assert!(
        page.body
            .contains(r#"<div class="z-grid-search" role="search">"#),
        "{}",
        page.body
    );
    assert!(page.body.contains(
        r#"data-zeyn-grid-search data-testid="banded.grid.search" aria-label="Search rows""#
    ));
    let q = serde_json::json!({ "with_total": true, "q": "c-1" });
    let r = send(&router, post("/_zeyn/grid", banded_call(q))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["total"], 10, "{}", r.body);
}

#[tokio::test]
async fn grids_without_search_drop_the_q_a_client_sends() {
    let q = serde_json::json!({ "with_total": true, "q": "c-1" });
    let r = send(&router(reader()), post("/_zeyn/grid", grid_call(q))).await;
    assert_eq!(r.json()["total"], 20, "{}", r.body);
    let page = send(&router(reader()), get("/items")).await;
    assert!(
        !page
            .body
            .contains("data-zeyn-grid-search")
    );
}

#[tokio::test]
async fn export_keeps_the_search_text() {
    let q = r#"{"q":"C-1"}"#;
    let r = send(
        &router(reader()),
        get(&format!(
            "/_zeyn/grid/export?screen=banded&node=0&q={}",
            q.replace('"', "%22")
                .replace('{', "%7B")
                .replace('}', "%7D")
        )),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body.trim_end().lines().count(), 11, "{}", r.body);
}

#[tokio::test]
async fn band_row_groups_shown_columns_under_colgroup_headers() {
    let page = send(&router(reader()), get("/banded")).await;
    let band = r#"<tr class="z-band-row"><td colspan="2" class="z-band-gap z-pin z-ps1 z-pin-edge-s" aria-colindex="1"></td><th scope="colgroup" colspan="2" class="z-band" aria-colindex="3">Stock</th><td colspan="1" class="z-band-gap" aria-colindex="5"></td></tr><tr>"#;
    assert!(page.body.contains(band), "{}", page.body);
    assert_eq!(page.body.matches("z-band-row").count(), 1);
}

#[tokio::test]
async fn moving_a_column_out_of_a_band_splits_it_and_pins_keep_their_side() {
    let call = serde_json::json!({
        "screen": "banded",
        "node": 0,
        "head": true,
        "query": {},
        "view": { "columns": [
            { "field": "price", "pin": "start" },
            { "field": "code" },
            { "field": "qty" },
            { "field": "added" }
        ] }
    });
    let r = send(&router(reader()), post("/_zeyn/grid", call)).await;
    let head = r.json()["head"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        head.contains(r#"<td colspan="1" class="z-band-gap z-pin z-ps1" aria-colindex="1"></td><th scope="colgroup" colspan="1" class="z-band z-pin z-ps2 z-pin-edge-s" aria-colindex="2">Stock</th><td colspan="1" class="z-band-gap" aria-colindex="3"></td><th scope="colgroup" colspan="1" class="z-band" aria-colindex="4">Stock</th>"#),
        "{head}"
    );
}

#[test]
fn search_shows_in_the_grid_schema_and_the_agent_tool() {
    let grid = DataGrid::of(search)
        .search()
        .column(Item::code)
        .done();
    assert_eq!(zeyn::Component::schema(&grid)["search"], true);
    assert_eq!(
        zeyn::GridSource::tool_schema(&grid)["properties"]["q"]["maxLength"],
        zeyn::MAX_SEARCH_QUERY
    );
}

/// Creates an item.
#[action(permission = "items.write")]
pub async fn create_item(item: Valid<Item>, store: Store) -> Result<()> {
    let mut item = item.into_inner();
    let mut rows = store.0.write().unwrap();
    item.id = ItemId(rows.len() as i64 + 1);
    rows.push(item);
    Ok(())
}

/// Deletes items by key.
#[action(permission = "items.write")]
pub async fn delete_items(ids: Vec<ItemId>, store: Store) -> Result<usize> {
    let mut rows = store.0.write().unwrap();
    let before = rows.len();
    rows.retain(|i| !ids.contains(&i.id));
    Ok(before - rows.len())
}

/// Saves an item in place.
#[action(permission = "items.write")]
pub async fn update_item(item: Valid<Item>, store: Store) -> Result<()> {
    let item = item.into_inner();
    let mut rows = store.0.write().unwrap();
    if let Some(row) = rows
        .iter_mut()
        .find(|r| r.id == item.id)
    {
        *row = item;
    }
    Ok(())
}

#[screen(route = "/rows-edit", permission = "items.read")]
pub fn rows_edit() -> Screen {
    Screen::new("Rows").body(
        DataGrid::of(search)
            .column(Item::code)
            .editable()
            .column(Item::qty)
            .editable()
            .column(Item::added)
            .editable()
            .column(Item::note)
            .add_with(create_item)
            .edit_with(update_item)
            .delete_with(delete_items, Item::id)
            .batch_edit(),
    )
}

fn writer() -> Identity {
    Identity::user("ayse").with_permissions(["items.read", "items.write"])
}

fn add_call(values: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": "rows_edit", "node": 0, "values": values })
}

#[tokio::test]
async fn add_row_creates_a_row_from_the_editable_columns() {
    let router = router(writer());
    let page = send(&router, get("/rows-edit")).await;
    assert!(
        page.body
            .contains(r#"data-zeyn-grid-add data-testid="rows_edit.grid.add""#),
        "{}",
        page.body
    );
    assert!(
        page.body
            .contains(r#"data-zeyn-delete>Delete</button>"#),
        "{}",
        page.body
    );
    assert!(
        page.body
            .contains(r#"data-zeyn-batch-bar"#)
    );
    assert!(
        page.body
            .contains(r#" data-zeyn-batch"#)
    );
    let r = send(
        &router,
        post(
            "/_zeyn/grid/add",
            add_call(serde_json::json!({ "code": "NEW-1", "qty": "7", "added": "2026-02-01" })),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let q = serde_json::json!({ "with_total": true, "filters": [] });
    let all = send(
        &router,
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "rows_edit", "node": 0, "query": q }),
        ),
    )
    .await;
    assert_eq!(all.json()["total"], 21, "{}", all.body);
}

#[tokio::test]
async fn add_row_refuses_fields_that_are_not_editable_and_validates() {
    let router = router(writer());
    let r = send(
        &router,
        post(
            "/_zeyn/grid/add",
            add_call(serde_json::json!({ "code": "X", "note": "hidden" })),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
    let r = send(
        &router,
        post(
            "/_zeyn/grid/add",
            add_call(serde_json::json!({ "code": "", "added": "2026-02-01" })),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.body);
}

#[tokio::test]
async fn add_row_needs_the_create_permission_and_a_grid_that_adds() {
    let r = send(
        &router(reader()),
        post(
            "/_zeyn/grid/add",
            add_call(serde_json::json!({ "code": "X" })),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
    let page = send(&router(reader()), get("/rows-edit")).await;
    assert!(!page.body.contains("data-zeyn-grid-add"));
    assert!(
        !page
            .body
            .contains("data-zeyn-batch-bar")
    );
    let other = serde_json::json!({ "screen": "list", "node": 0, "values": { "code": "X" } });
    let r = send(&router(writer()), post("/_zeyn/grid/add", other)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.body);
}

#[tokio::test]
async fn add_row_needs_the_csrf_token() {
    let req = Request::post("/_zeyn/grid/add")
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            add_call(serde_json::json!({ "code": "X" })).to_string(),
        ))
        .unwrap();
    let r = send(&router(writer()), req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
}

#[tokio::test]
async fn delete_with_runs_the_action_with_the_picked_keys() {
    let router = router(writer());
    let call =
        serde_json::json!({ "screen": "rows_edit", "node": 0, "action": 0, "keys": ["3", "4"] });
    let r = send(&router, post("/_zeyn/grid/bulk", call)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let q = serde_json::json!({ "with_total": true });
    let all = send(
        &router,
        post(
            "/_zeyn/grid",
            serde_json::json!({ "screen": "rows_edit", "node": 0, "query": q }),
        ),
    )
    .await;
    assert_eq!(all.json()["total"], 18, "{}", all.body);
}
