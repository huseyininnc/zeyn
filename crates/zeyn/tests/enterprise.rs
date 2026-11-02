//! Audit, privacy, session safety, health, import, jobs and workflow.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{FieldChange, Identity};

#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    #[rule(required, max_len = 60)]
    pub name: String,
    pub credit: Money,
    #[readonly]
    pub score: i64,
    #[version]
    pub version: i64,
}

#[derive(Clone, Default)]
pub struct Customers(Arc<RwLock<Vec<Customer>>>);

impl Service for Customers {}

impl Customers {
    fn seeded() -> Self {
        let rows = (1..=5)
            .map(|i| Customer {
                id: CustomerId(i),
                name: format!("Customer {i}"),
                credit: Money::tl(i * 100_000),
                score: i,
                version: 1,
            })
            .collect();
        Customers(Arc::new(RwLock::new(rows)))
    }

    fn get(&self, id: i64) -> Option<Customer> {
        self.0.read().unwrap().iter().find(|c| c.id.0 == id).cloned()
    }
}

#[action(permission = "crm.read")]
pub async fn find_customers(q: GridQuery, store: Customers) -> Result<Page<Customer>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "crm.write", audit)]
pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
    let mut rows = store.0.write().unwrap();
    let current = rows.iter_mut().find(|r| r.id == c.id).ok_or(Error::NotFound)?;
    if current.version != c.version {
        return Err(Error::Conflict);
    }
    ctx.record_changes(&*current, &*c);
    let mut saved = c.into_inner();
    saved.score = current.score;
    saved.version += 1;
    *current = saved.clone();
    Ok(saved)
}

#[params]
pub struct CustomerParams {
    pub id: i64,
}

#[screen(route = "/customers", permission = "crm.read")]
pub fn customers() -> Screen {
    Screen::new("Customers").body(Grid::of(find_customers).column(Customer::name).column(Customer::credit))
}

#[screen(permission = "crm.write")]
pub fn customer(params: CustomerParams, store: Customers) -> Result<Screen> {
    let c = store.get(params.id).ok_or(Error::NotFound)?;
    Ok(Screen::window("Customer").body(
        Form::of(c).field(Customer::name).field(Customer::credit).field(Customer::score).submit("Save", save_customer),
    ))
}

fn crm_writer() -> Identity {
    Identity::user("ayse").with_permissions(["crm.read", "crm.write"])
}

fn crm_app(identity: Identity, audit: MemoryAudit) -> App {
    App::new("Test")
        .service(Customers::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .feature(Feature::new("crm").screen(customers).screen(customer).api(find_customers).api(save_customer))
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
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

fn api(path: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(path)
        .header("x-user", "1")
        .header(header::AUTHORIZATION, "Bearer t")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn change(field: &'static str, old: &str, new: &str) -> FieldChange {
    FieldChange { field, old: Some(old.into()), new: Some(new.into()) }
}

#[tokio::test]
async fn form_save_records_the_record_key_and_changed_fields_from_the_loaded_record() {
    let audit = MemoryAudit::default();
    let router = crm_app(crm_writer(), audit.clone()).router().unwrap();
    let values = serde_json::json!({ "name": "Renamed", "credit": "2000", "score": "999", "id": "4", "version": "1" });
    let r = send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "customer", "params": { "id": 2 }, "node": 0, "values": values }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let entries = audit.entries();
    assert_eq!(entries.len(), 1);
    let e = &entries[0];
    assert_eq!(e.model, Some("Customer"));
    assert_eq!(e.record.as_deref(), Some("2"));
    assert_eq!(e.changes, vec![change("name", "Customer 2", "Renamed")]);
    let line = serde_json::to_value(e).unwrap();
    assert_eq!(line["changes"][0]["old"], "Customer 2");
}

#[tokio::test]
async fn api_save_records_changes_against_the_stored_record_not_the_request() {
    let audit = MemoryAudit::default();
    let router = crm_app(crm_writer(), audit.clone()).router().unwrap();
    let body = serde_json::json!({ "id": 3, "name": "Customer 3", "credit": { "minor": 123456, "currency": "Try" }, "score": 77, "version": 1 });
    let r = send(&router, api("/api/v1/crm/save_customer", body)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let e = &audit.entries()[0];
    assert_eq!(e.model, Some("Customer"));
    assert_eq!(e.record.as_deref(), Some("3"));
    assert_eq!(e.changes, vec![change("credit", "3000.00 TRY", "1234.56 TRY"), change("score", "3", "77")]);
}

#[tokio::test]
async fn failed_saves_are_recorded_without_inventing_changes() {
    let audit = MemoryAudit::default();
    let router = crm_app(crm_writer(), audit.clone()).router().unwrap();
    let body = serde_json::json!({ "id": 3, "name": "X", "credit": { "minor": 1, "currency": "Try" }, "score": 0, "version": 9 });
    let r = send(&router, api("/api/v1/crm/save_customer", body)).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    let e = &audit.entries()[0];
    assert!(!e.ok);
    assert_eq!(e.record.as_deref(), Some("3"));
    assert!(e.changes.is_empty());
}
