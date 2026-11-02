//! AI policy: tool descriptions from doc comments, `ai = "..."`, `ai = false`, `confirm`, and the
//! `Mcp` switch for the MCP endpoint and WebMCP tools.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value as Json, json};
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Ai, Identity};

#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    #[rule(required)]
    pub name: String,
    #[mask(last4)]
    pub iban: String,
}

#[model]
pub struct ArchiveRequest {
    #[rule(required)]
    pub name: String,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Customer>>>);

impl Service for Store {}

impl Store {
    fn seeded() -> Self {
        let rows = vec![
            Customer { id: CustomerId(1), name: "Ada".into(), iban: "TR12 0006 1000 0000 1234 5678 90".into() },
            Customer { id: CustomerId(2), name: "Can".into(), iban: "TR98 0001 2000 0000 9876 5432 10".into() },
        ];
        Store(Arc::new(RwLock::new(rows)))
    }
}

/// Customers for lists and the API.
///
/// This second paragraph is not part of the tool description.
#[action(permission = "crm.read", ai = "Filter by name when the user names a customer.")]
pub async fn search_customers(q: DataQuery, store: Store) -> Result<Page<Customer>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

/// Internal search that models must never see.
#[action(permission = "crm.read", ai = false)]
pub async fn internal_search(q: DataQuery, store: Store) -> Result<Page<Customer>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

/// Saves a new customer.
#[action(permission = "crm.write")]
pub async fn save_customer(c: Valid<Customer>, store: Store) -> Result<Customer> {
    let mut c = c.into_inner();
    let mut rows = store.0.write().unwrap();
    c.id = CustomerId(rows.len() as i64 + 1);
    rows.push(c.clone());
    Ok(c)
}

/// Archives a customer by name. The customer stays in reports.
#[action(permission = "crm.write", confirm)]
pub async fn archive_customer(r: Valid<ArchiveRequest>, store: Store) -> Result<()> {
    store.0.write().unwrap().retain(|c| c.name != r.name);
    Ok(())
}

/// Rebuilds the search index.
#[action(permission = "crm.write", ai = false)]
pub async fn reindex(_: ()) -> Result<()> {
    Ok(())
}

/// Every customer of the company.
#[screen(route = "/customers", permission = "crm.read")]
pub fn customers() -> Screen {
    Screen::new("Customers").body(DataGrid::of(search_customers).column(Customer::name).filter().column(Customer::iban))
}

#[screen(route = "/internal", permission = "crm.read")]
pub fn internal() -> Screen {
    Screen::new("Internal").body(DataGrid::of(internal_search).column(Customer::name))
}

/// A screen for people only.
#[screen(route = "/hidden", permission = "crm.read", ai = false)]
pub fn hidden() -> Screen {
    Screen::new("Hidden").body(DataGrid::of(search_customers).column(Customer::name))
}

#[screen(route = "/new", permission = "crm.write", ai = "Ask for the name before saving.")]
pub fn new_customer() -> Screen {
    Screen::new("New customer").body(Form::of(Customer::default()).field(Customer::name).submit("Save", save_customer))
}

#[screen(route = "/archive", permission = "crm.write")]
pub fn archive() -> Screen {
    Screen::new("Archive").body(Form::of(ArchiveRequest::default()).field(ArchiveRequest::name).submit("Archive", archive_customer))
}

fn app(mcp: Option<Mcp>, audit: MemoryAudit) -> App {
    let identity = Identity::user("ayse").with_permissions(["crm.read", "crm.write", "customer.iban.unmask"]);
    let app = App::new("Crm")
        .service(Store::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .api_docs(ApiDocs::open())
        .feature(
            Feature::new("crm")
                .screen(customers)
                .screen(internal)
                .screen(hidden)
                .screen(new_customer)
                .screen(archive)
                .api(search_customers)
                .api(internal_search)
                .api(save_customer)
                .api(archive_customer)
                .api(reindex),
        );
    match mcp {
        Some(m) => app.mcp(m),
        None => app,
    }
}

fn router(mcp: Option<Mcp>) -> Router {
    app(mcp, MemoryAudit::default()).router().unwrap()
}

struct Reply {
    status: StatusCode,
    body: String,
}

impl Reply {
    fn json(&self) -> Json {
        serde_json::from_str(&self.body).unwrap()
    }
}

async fn send(router: &Router, req: Request<Body>) -> Reply {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    Reply { status, body }
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).header("x-user", "1").body(Body::empty()).unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn post(path: &str, body: Json) -> Request<Body> {
    Request::post(path)
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn rpc(body: Json) -> Request<Body> {
    Request::post("/_zeyn/mcp")
        .header("x-user", "1")
        .header(header::AUTHORIZATION, "Bearer test")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn mcp_tools(router: &Router) -> Vec<Json> {
    let r = send(router, rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    r.json()["result"]["tools"].as_array().cloned().unwrap_or_default()
}

fn names(tools: &[Json]) -> Vec<String> {
    tools.iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
}

async fn call(router: &Router, name: &str, args: Json) -> Json {
    let r = send(router, rpc(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": args } }))).await;
    r.json()["result"].clone()
}

#[test]
fn macro_options_land_in_the_metadata() {
    let meta = <search_customers as Action>::META;
    assert_eq!(meta.doc, "Customers for lists and the API.");
    assert_eq!(meta.ai, Ai { offered: true, hint: "Filter by name when the user names a customer.", confirm: false });
    assert!(<archive_customer as Action>::META.ai.confirm);
    assert!(!<reindex as Action>::META.ai.offered);
    assert_eq!(<customers as ScreenDef>::META.doc, "Every customer of the company.");
    assert!(!<hidden as ScreenDef>::META.ai.offered);
    assert_eq!(<internal as ScreenDef>::META.doc, "");
    assert_eq!(Ai::default(), Ai::DEFAULT);
}

#[tokio::test]
async fn mcp_is_off_by_default() {
    let router = router(None);
    assert_eq!(send(&router, get("/_zeyn/tools?screen=customers")).await.status, StatusCode::NOT_FOUND);
    let r = send(&router, rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&router, post("/_zeyn/form", json!({ "screen": "new_customer", "node": 0, "values": { "name": "Deniz" }, "via_agent": true }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "an agent form save needs WebMCP writes");
    assert!(app(None, MemoryAudit::default()).describe()["mcp"].is_null());
}

#[tokio::test]
async fn descriptions_come_from_doc_comments_and_ai_guidance() {
    let tools = mcp_tools(&router(Some(Mcp::on()))).await;
    let search = tools.iter().find(|t| t["name"] == "customers_0_search").unwrap();
    let text = search["description"].as_str().unwrap();
    assert!(text.starts_with("Search the \"Customers\" list."), "{text}");
    assert!(text.contains("Every customer of the company."), "{text}");
    assert!(text.contains("Customers for lists and the API."), "{text}");
    assert!(text.contains("Filter by name when the user names a customer."), "{text}");
    assert!(!text.contains("second paragraph"), "{text}");
    let save = tools.iter().find(|t| t["name"] == "new_customer_0_save").unwrap();
    assert!(save["description"].as_str().unwrap().contains("Saves a new customer. Ask for the name before saving."));
    let api = tools.iter().find(|t| t["name"] == "api_crm_save_customer").unwrap();
    assert_eq!(api["description"], "Saves a new customer.");
}

#[tokio::test]
async fn ai_false_hides_screens_and_actions_everywhere() {
    let router = router(Some(Mcp::on()));
    let listed = names(&mcp_tools(&router).await);
    assert!(listed.contains(&"customers_0_search".to_string()));
    for name in ["hidden_0_search", "internal_0_search", "api_crm_internal_search", "api_crm_reindex"] {
        assert!(!listed.contains(&name.to_string()), "{name} is listed");
        let r = call(&router, name, json!({})).await;
        assert_eq!(r["isError"], true, "{name} ran");
    }
    let r = send(&router, get("/_zeyn/tools?screen=hidden")).await;
    assert_eq!(r.json()["tools"], json!([]));
    let r = send(&router, get("/_zeyn/tools?screen=internal")).await;
    assert_eq!(r.json()["tools"], json!([]));

    let doc = send(&router, get("/api/v1/openapi.json")).await.json();
    assert_eq!(doc["paths"]["/api/v1/crm/reindex"]["post"]["x-zeyn-ai"], json!(false));
    assert_eq!(doc["paths"]["/api/v1/crm/archive_customer"]["post"]["x-zeyn-ai"]["confirm"], true);
    assert_eq!(doc["paths"]["/api/v1/crm/search_customers"]["post"]["x-zeyn-ai"]["writes"], false);
    assert!(doc["paths"]["/api/v1/crm/save_customer"]["post"]["description"].as_str().unwrap().starts_with("Saves a new customer."));
}

#[tokio::test]
async fn confirm_actions_are_never_run_over_the_mcp_endpoint() {
    let router = router(Some(Mcp::on()));
    let listed = names(&mcp_tools(&router).await);
    assert!(!listed.contains(&"archive_0_save".to_string()));
    assert!(!listed.contains(&"api_crm_archive_customer".to_string()));
    for name in ["archive_0_save", "api_crm_archive_customer"] {
        let r = call(&router, name, json!({ "name": "Ada" })).await;
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"].as_str().unwrap().contains("confirmation"), "{r}");
    }
    let r = call(&router, "customers_0_search", json!({})).await;
    assert!(r["content"][0]["text"].as_str().unwrap().contains("Ada"), "the archive did not run");
}

#[tokio::test]
async fn webmcp_confirm_needs_the_users_confirmation() {
    let router = router(Some(Mcp::on()));
    let r = send(&router, get("/_zeyn/tools?screen=archive")).await;
    assert_eq!(r.json()["tools"][0]["confirm"], true);
    assert!(r.json()["texts"]["ok"].is_string());

    let save = |confirmed: bool| post("/_zeyn/form", json!({ "screen": "archive", "node": 0, "values": { "name": "Ada" }, "via_agent": true, "confirmed": confirmed }));
    let r = send(&router, save(false)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["error"], "confirmation_required");
    let r = send(&router, save(true)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    let r = send(&router, post("/_zeyn/form", json!({ "screen": "hidden", "node": 0, "values": {}, "via_agent": true }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn read_only_lists_no_writes_and_refuses_them() {
    let router = router(Some(Mcp::read_only()));
    let listed = names(&mcp_tools(&router).await);
    assert_eq!(listed, ["customers_0_search", "api_crm_search_customers"]);
    let r = call(&router, "api_crm_save_customer", json!({ "id": 0, "name": "Deniz", "iban": "" })).await;
    assert_eq!(r["isError"], true);
    let r = send(&router, post("/_zeyn/form", json!({ "screen": "new_customer", "node": 0, "values": { "name": "Deniz" }, "via_agent": true }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn each_door_can_be_closed_on_its_own() {
    let browser_only = router(Some(Mcp::on().server(false)));
    assert_eq!(send(&browser_only, get("/_zeyn/tools?screen=customers")).await.status, StatusCode::OK);
    let r = send(&browser_only, rpc(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    let router = router(Some(Mcp::on().webmcp(false)));
    assert_eq!(send(&router, get("/_zeyn/tools?screen=customers")).await.status, StatusCode::NOT_FOUND);
    assert!(!mcp_tools(&router).await.is_empty());
    let r = send(&router, post("/_zeyn/form", json!({ "screen": "new_customer", "node": 0, "values": { "name": "Deniz" }, "via_agent": true }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn agents_never_see_masked_values_and_every_call_is_audited() {
    let audit = MemoryAudit::default();
    let router = app(Some(Mcp::on()), audit.clone()).router().unwrap();
    let r = call(&router, "api_crm_search_customers", json!({})).await;
    let text = r["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("1234 5678 90"), "the agent got the raw value: {text}");
    assert!(text.contains("•••• 8 90"), "{text}");

    let direct = Request::post("/api/v1/crm/search_customers")
        .header("x-user", "1")
        .header(header::AUTHORIZATION, "Bearer test")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let r = send(&router, direct).await;
    assert!(r.body.contains("1234 5678 90"), "the user may still reveal it through the API");

    call(&router, "customers_0_search", json!({})).await;
    let entries = audit.entries();
    assert!(entries.iter().filter(|e| e.via_agent).count() >= 2, "{entries:?}");
    assert!(entries.iter().any(|e| e.via_agent && e.action == Some("search_customers")));
}
