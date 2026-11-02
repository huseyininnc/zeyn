use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{GuardResult, I18n, Identity};
use zeyn::prelude::TimePolicy;

/// Product used by the server tests.
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    #[version]
    pub version: i64,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Product>>>);

impl Service for Store {}

impl Store {
    fn seeded() -> Self {
        let rows = (1..=30)
            .map(|i| Product { id: ProductId(i), code: format!("P-{i:02}"), price: Money::tl(i * 100), stock: i, version: 1 })
            .collect();
        Store(Arc::new(RwLock::new(rows)))
    }

    fn get(&self, id: i64) -> Product {
        self.0.read().unwrap().iter().find(|p| p.id.0 == id).cloned().unwrap()
    }
}

#[action(permission = "stock.read")]
pub async fn search(q: GridQuery, store: Store) -> Result<Page<Product>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "stock.write", audit)]
pub async fn save(p: Valid<Product>, store: Store) -> Result<Product> {
    let mut rows = store.0.write().unwrap();
    let current = rows.iter_mut().find(|r| r.id == p.id).ok_or(Error::NotFound)?;
    if current.version != p.version {
        return Err(Error::Conflict);
    }
    let mut saved = p.into_inner();
    saved.version += 1;
    *current = saved.clone();
    Ok(saved)
}

#[params]
pub struct EditParams {
    pub id: i64,
}

#[screen(route = "/products", permission = "stock.read")]
pub fn list() -> Screen {
    Screen::new("Products").body(Grid::of(search).paging(Paging::pages(10)).column(Product::code).filter().column(Product::price))
}

#[screen(permission = "stock.write")]
pub fn edit(params: EditParams, store: Store) -> Screen {
    Screen::window("Product").body(
        Form::of(store.get(params.id))
            .field(Product::code)
            .field(Product::price)
            .field(Product::stock)
            .submit("Save", save)
            .on_success(|_, ui| ui.toast("Saved")),
    )
}

#[screen(route = "/closed", public)]
pub fn closed() -> Screen {
    Screen::new("Closed")
}

#[screen(route = "/clock", public)]
pub fn clock(_: (), ctx: Ctx) -> Screen {
    Screen::new(Text::from(format!("zone={} today={}", ctx.time_zone().name(), ctx.today())))
}

fn weekend_only(_: &Ctx) -> GuardResult {
    GuardResult::Deny("Closed today".into())
}

fn app(identity: Identity, audit: MemoryAudit, agents: bool) -> App {
    let i18n = I18n::new("en").catalog("tr", r#"{ "Product": { "price": "Fiyat", "code": "Kod" } }"#).unwrap();
    let mut app = App::new("Test")
        .i18n(i18n)
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .feature(Feature::new("stock").screen(list).screen(edit).screen(clock).api(search).api(save))
        .feature(Feature::new("other").guard(weekend_only).screen(closed));
    if agents {
        app = app.agents(Agents::read_write());
    }
    app
}

fn router(identity: Identity) -> Router {
    app(identity, MemoryAudit::default(), false).router().unwrap()
}

fn writer() -> Identity {
    Identity::user("ayse").with_permissions(["stock.read", "stock.write"])
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

fn get(path: &str) -> Request<Body> {
    Request::get(path).header("x-user", "1").body(Body::empty()).unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn post(path: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(path)
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn page_renders_first_page_with_security_headers_and_csrf_cookie() {
    let r = send(&router(writer()), get("/products")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.contains(r#"<meta name="zeyn-csrf" content=""#));
    assert!(r.body.contains(r#"data-zeyn-total="30""#));
    assert!(r.body.contains("<td>P-01</td>"));
    assert!(!r.body.contains("<td>P-11</td>"));
    let cookie = r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(cookie.starts_with("zeyn_csrf=") && cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    assert!(r.headers.get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap().contains("script-src 'self'"));
    assert_eq!(r.headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
}

#[tokio::test]
async fn posts_without_matching_csrf_token_are_refused() {
    let router = router(writer());
    let mut req = post("/_zeyn/grid", serde_json::json!({ "screen": "list", "node": 0 }));
    req.headers_mut().insert("x-zeyn-csrf", "f".repeat(64).parse().unwrap());
    let r = send(&router, req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["error"], "forbidden");
}

#[tokio::test]
async fn grid_endpoint_filters_sorts_and_pages() {
    let body = serde_json::json!({
        "screen": "list", "node": 0,
        "query": { "page": { "type": "offset", "offset": 0, "limit": 2 }, "sort": [{ "field": "price", "desc": true }], "filters": [{ "field": "code", "contains": "p-1" }], "with_total": true }
    });
    let r = send(&router(writer()), post("/_zeyn/grid", body)).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["total"], 10);
    assert_eq!(j["count"], 2);
    let rows = j["rows"].as_str().unwrap();
    assert!(rows.find("P-19").unwrap() < rows.find("P-18").unwrap());
}

#[tokio::test]
async fn form_reports_unreadable_values_and_rule_errors_together_in_the_users_language() {
    let mut req = post(
        "/_zeyn/form",
        serde_json::json!({ "screen": "edit", "params": { "id": 3 }, "node": 0, "values": { "code": "", "price": "abc", "version": "1" } }),
    );
    req.headers_mut().insert(header::ACCEPT_LANGUAGE, "tr-TR,tr;q=0.9".parse().unwrap());
    let r = send(&router(writer()), req).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let errors = r.json()["errors"].clone();
    assert_eq!(errors[0]["field"], "price");
    assert_eq!(errors[0]["message"], "Fiyat geçersiz bir değer içeriyor.");
    assert_eq!(errors[1]["field"], "code");
    assert_eq!(errors[1]["message"], "Kod alanı zorunludur.");
}

#[tokio::test]
async fn form_takes_only_editable_fields_and_detects_stale_versions() {
    let audit = MemoryAudit::default();
    let router = app(writer(), audit.clone(), false).router().unwrap();
    let values = serde_json::json!({ "id": "99", "stock": "5000", "code": "NEW", "price": "12.50", "version": "1" });
    let r = send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "edit", "params": { "id": 3 }, "node": 0, "values": values }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["effects"][0]["message"], "Saved");

    let grid = send(&router, post("/_zeyn/grid", serde_json::json!({ "screen": "list", "node": 0, "query": { "filters": [{ "field": "code", "contains": "new" }] } }))).await;
    let rows = grid.json()["rows"].as_str().unwrap().to_string();
    assert!(rows.contains("<td>NEW</td>") && rows.contains("₺12.50"), "{rows}");

    let again = serde_json::json!({ "screen": "edit", "params": { "id": 3 }, "node": 0, "values": { "code": "AGAIN", "price": "1", "version": "1" } });
    let r = send(&router, post("/_zeyn/form", again)).await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    let entries = audit.entries();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].ok && !entries[1].ok);
    assert_eq!(entries[1].error, Some("conflict"));
    assert_eq!(entries[0].user.as_deref(), Some("ayse"));
}

#[tokio::test]
async fn permissions_and_guards_are_enforced_on_the_server() {
    let reader = router(Identity::user("ali").with_permissions(["stock.read"]));
    let r = send(&reader, post("/_zeyn/form", serde_json::json!({ "screen": "edit", "params": { "id": 1 }, "node": 0, "values": {} }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = send(&reader, get("/_zeyn/screen/edit?params=%7B%22id%22%3A1%7D")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let nobody = router(Identity::user("x"));
    let r = send(&nobody, get("/products")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(!r.body.contains("P-01"));

    let r = send(&router(writer()), get("/closed")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.body.contains("Closed today"));
}

#[tokio::test]
async fn menu_hides_screens_the_user_cannot_open() {
    let app = |id: Identity| {
        App::new("T")
            .service(Store::seeded())
            .auth(move |_: &HeaderMap| Some(id.clone()))
            .feature(Feature::new("stock").screen(list).screen(edit).menu(|m| m.group("Stock", Icon::Box).item("Products", list)))
            .feature(Feature::new("other").screen(closed).menu(|m| m.group("Other", Icon::Home).item("Closed", closed)))
            .router()
            .unwrap()
    };
    let r = send(&app(Identity::user("x")), get("/closed")).await;
    assert!(r.body.contains(r#"href="/closed" aria-current="page""#));
    assert!(!r.body.contains(r#"href="/products""#));
}

#[tokio::test]
async fn locale_can_be_switched_only_to_offered_languages() {
    let router = router(writer());
    let r = send(&router, post("/_zeyn/locale", serde_json::json!({ "locale": "tr" }))).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap().starts_with("zeyn_locale=tr;"));
    let r = send(&router, post("/_zeyn/locale", serde_json::json!({ "locale": "xx" }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn check_reports_missing_services_and_bad_home() {
    let problems = App::new("T").feature(Feature::new("stock").screen(edit)).home(edit).check().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("needs service") && p.contains("Store")));
    assert!(problems.iter().any(|p| p.contains("home screen `edit`")));
}

#[tokio::test]
async fn agent_tools_are_off_by_default_and_mcp_needs_an_authorization_header() {
    let r = send(&router(writer()), get("/_zeyn/tools?screen=list")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    let router = app(writer(), MemoryAudit::default(), true).router().unwrap();
    let r = send(&router, get("/_zeyn/tools?screen=list")).await;
    assert_eq!(r.json()["tools"][0]["name"], "list_0_search");

    let rpc = |auth: bool, body: serde_json::Value| {
        let mut b = Request::post("/_zeyn/mcp").header("x-user", "1").header(header::CONTENT_TYPE, "application/json");
        if auth {
            b = b.header(header::AUTHORIZATION, "Bearer test");
        }
        b.body(Body::from(body.to_string())).unwrap()
    };
    let r = send(&router, rpc(false, serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    let r = send(&router, rpc(true, serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    let names: Vec<String> = r.json()["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, ["list_0_search"]);

    let call = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "list_0_search", "arguments": { "page": { "type": "offset", "offset": 0, "limit": 1 } } } });
    let r = send(&router, rpc(true, call)).await;
    assert_eq!(r.json()["result"]["isError"], false);
    assert!(r.json()["result"]["content"][0]["text"].as_str().unwrap().contains("P-01"));
}

#[tokio::test]
async fn unknown_paths_and_screens_are_404() {
    let router = router(writer());
    assert_eq!(send(&router, get("/nope")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, get("/_zeyn/screen/nope")).await.status, StatusCode::NOT_FOUND);
    let r = send(&router, post("/_zeyn/grid", serde_json::json!({ "screen": "list", "node": 7 }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn time_zone_comes_from_profile_then_manual_choice_then_browser_then_default() {
    let noon_utc = DateTime::parse("2024-01-05T22:30:00Z").unwrap();
    let build = |identity: Identity, policy: TimePolicy| {
        App::new("T")
            .auth(move |_: &HeaderMap| Some(identity.clone()))
            .time_policy(policy)
            .fixed_now(noon_utc)
            .feature(Feature::new("t").screen(clock))
            .router()
            .unwrap()
    };
    let user = TimePolicy::User { default: TimeZone::named("Europe/London").unwrap() };
    let page = |cookies: &str| {
        Request::get("/clock").header(header::COOKIE, cookies.to_string()).body(Body::empty()).unwrap()
    };

    let r = send(&build(Identity::user("a"), user), page("")).await;
    assert!(r.body.contains("zone=Europe/London today=2024-01-05"), "{}", r.body);

    let r = send(&build(Identity::user("a"), user), page("zeyn_tz=Europe%2FIstanbul")).await;
    assert!(r.body.contains("zone=Europe/Istanbul today=2024-01-06"));
    assert!(r.body.contains(r#"<meta name="zeyn-tz" content="Europe/Istanbul">"#));

    let r = send(&build(Identity::user("a"), user), page("zeyn_tz=Europe%2FIstanbul; zeyn_tz_manual=America%2FNew_York")).await;
    assert!(r.body.contains("zone=America/New_York"));

    let profile = Identity::user("a").with_time_zone("Asia/Tokyo");
    let r = send(&build(profile.clone(), user), page("zeyn_tz_manual=America%2FNew_York")).await;
    assert!(r.body.contains("zone=Asia/Tokyo"));

    let org = TimePolicy::Organization(TimeZone::named("Europe/Istanbul").unwrap());
    let r = send(&build(profile, org), page("zeyn_tz=America%2FNew_York")).await;
    assert!(r.body.contains("zone=Europe/Istanbul"));

    let r = send(&build(Identity::user("a"), user), page("zeyn_tz=Not%2FAZone")).await;
    assert!(r.body.contains("zone=Europe/London"));
}

#[tokio::test]
async fn manual_time_zone_choice_is_validated_and_can_be_cleared() {
    let router = router(writer());
    let r = send(&router, post("/_zeyn/timezone", serde_json::json!({ "zone": "Asia/Kolkata" }))).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap().starts_with("zeyn_tz_manual=Asia%2FKolkata;"));
    let r = send(&router, post("/_zeyn/timezone", serde_json::json!({ "zone": "Nowhere/Land" }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&router, post("/_zeyn/timezone", serde_json::json!({ "zone": "" }))).await;
    assert!(r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap().contains("Max-Age=0"));
}

fn api(body: serde_json::Value, path: &str, auth: bool) -> Request<Body> {
    let mut b = Request::post(path).header("x-user", "1").header(header::CONTENT_TYPE, "application/json");
    if auth {
        b = b.header(header::AUTHORIZATION, "Bearer t");
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn actions_published_as_api_need_authorization_and_return_problem_details() {
    let router = router(writer());
    let r = send(&router, api(serde_json::json!({}), "/api/v1/stock/search", false)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.headers.get(header::CONTENT_TYPE).unwrap(), "application/problem+json");

    let q = serde_json::json!({ "page": { "type": "offset", "offset": 0, "limit": 2 }, "with_total": true });
    let r = send(&router, api(q, "/api/v1/stock/search", true)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["total"], 30);
    assert_eq!(r.json()["items"][1]["code"], "P-02");

    let bad = serde_json::json!({ "id": 3, "code": "", "price": { "minor": -5, "currency": "Try" }, "stock": 0, "version": 1 });
    let r = send(&router, api(bad, "/api/v1/stock/save", true)).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let body = r.json();
    let fields: Vec<&str> = body["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["code", "price"]);
    assert_eq!(r.json()["type"], "urn:zeyn:problem:validation");

    let stale = serde_json::json!({ "id": 3, "code": "X", "price": { "minor": 5, "currency": "Try" }, "stock": 0, "version": 7 });
    let r = send(&router, api(stale, "/api/v1/stock/save", true)).await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    let ok = serde_json::json!({ "id": 3, "code": "X", "price": { "minor": 5, "currency": "Try" }, "stock": 0, "version": 1 });
    let r = send(&router, api(ok, "/api/v1/stock/save", true)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["version"], 2);

    let reader = super_reader();
    let r = send(&reader, api(serde_json::json!({}), "/api/v1/stock/save", true)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

fn super_reader() -> Router {
    router(Identity::user("ali").with_permissions(["stock.read"]))
}

#[tokio::test]
async fn openapi_describes_published_actions_from_the_models() {
    let hidden = send(&router(writer()), Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap()).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let docs_router = app(writer(), MemoryAudit::default(), false).api_docs(ApiDocs::open()).router().unwrap();
    let r = send(&docs_router, Request::get("/api/v1/openapi.json").body(Body::empty()).unwrap()).await;
    let doc = r.json();
    let page = send(&docs_router, Request::get("/api/v1/docs").body(Body::empty()).unwrap()).await;
    assert!(page.body.contains("<code>POST /api/v1/stock/save</code>"));
    assert!(page.body.contains("Permission: stock.write"));
    assert!(!page.body.contains("<script>"));
    assert_eq!(doc["openapi"], "3.1.0");
    let op = &doc["paths"]["/api/v1/stock/save"]["post"];
    assert_eq!(op["description"], "Permission: stock.write");
    let schema = &op["requestBody"]["content"]["application/json"]["schema"];
    assert_eq!(schema["properties"]["code"]["maxLength"], 20);
    assert_eq!(schema["properties"]["stock"]["readOnly"], true);
    assert_eq!(op["responses"]["200"]["content"]["application/json"]["schema"]["title"], "Product");
    assert!(doc["paths"]["/api/v1/stock/search"]["post"]["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["items"].is_object());
}

#[tokio::test]
async fn check_reports_missing_services_of_published_actions() {
    let problems = App::new("T").feature(Feature::new("stock").api(search)).check().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("api `search` needs service")));
}
