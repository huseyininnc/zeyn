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

#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    #[rule(required)]
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[version]
    pub version: i64,
}

#[derive(Clone, Default)]
pub struct People(Arc<RwLock<Vec<Person>>>);

impl Service for People {}

impl People {
    fn seeded() -> Self {
        let rows = (1..=3)
            .map(|i| Person {
                id: PersonId(i),
                name: format!("Person {i}"),
                national_id: format!("1234567890{i}"),
                email: Some(format!("person{i}@example.com")),
                version: 1,
            })
            .collect();
        People(Arc::new(RwLock::new(rows)))
    }

    fn get(&self, id: i64) -> Option<Person> {
        self.0.read().unwrap().iter().find(|c| c.id.0 == id).cloned()
    }
}

#[action(permission = "hr.read")]
pub async fn find_people(q: GridQuery, store: People) -> Result<Page<Person>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "hr.read")]
pub async fn get_person(id: i64, store: People) -> Result<Person> {
    store.get(id).ok_or(Error::NotFound)
}

#[action(permission = "hr.write")]
pub async fn save_person(p: Valid<Person>, store: People) -> Result<Person> {
    let mut rows = store.0.write().unwrap();
    let current = rows.iter_mut().find(|r| r.id == p.id).ok_or(Error::NotFound)?;
    let mut saved = p.into_inner();
    saved.version += 1;
    *current = saved.clone();
    Ok(saved)
}

#[screen(route = "/people", permission = "hr.read")]
pub fn people() -> Screen {
    Screen::new("People").body(
        Grid::of(find_people).column(Person::name).filter().column(Person::national_id).filter().column(Person::email),
    )
}

#[screen(permission = "hr.read")]
pub fn person(params: CustomerParams, store: People) -> Result<Screen> {
    let p = store.get(params.id).ok_or(Error::NotFound)?;
    Ok(Screen::window("Person").body(
        Form::of(p).field(Person::name).field(Person::national_id).field(Person::email).submit("Save", save_person),
    ))
}

fn hr_app(identity: Identity, audit: MemoryAudit) -> Router {
    App::new("Test")
        .service(People::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .feature(Feature::new("hr").screen(people).screen(person).api(find_people).api(get_person).api(save_person))
        .router()
        .unwrap()
}

fn clerk() -> Identity {
    Identity::user("clerk").with_permissions(["hr.read", "hr.write"])
}

fn officer() -> Identity {
    clerk().with_permissions(["person.national_id.unmask", "hr.contact.unmask"])
}

#[tokio::test]
async fn masked_fields_never_reach_users_without_the_unmask_permission() {
    let router = hr_app(clerk(), MemoryAudit::default());
    let page = send(&router, get("/people")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("•••• 8901"), "{}", page.body);
    assert!(page.body.contains("p••••@example.com"));
    assert!(!page.body.contains("12345678901") && !page.body.contains("person1@"));
    assert!(!page.body.contains("data-zeyn-reveal"));

    let form = send(&router, get("/_zeyn/screen/person?params=%7B%22id%22%3A1%7D")).await;
    assert!(form.body.contains(r#"readonly data-zeyn-masked value="•••• 8901""#), "{}", form.body);
    assert!(!form.body.contains("12345678901"));

    let one = send(&router, api("/api/v1/hr/get_person", serde_json::json!(2))).await;
    assert_eq!(one.json()["national_id"], "•••• 8902");
    assert_eq!(one.json()["email"], "p••••@example.com");
    let list = send(&router, api("/api/v1/hr/find_people", serde_json::json!({}))).await;
    assert_eq!(list.json()["items"][2]["national_id"], "•••• 8903");
}

#[tokio::test]
async fn masked_fields_cannot_be_searched_or_overwritten_without_the_permission() {
    let router = hr_app(clerk(), MemoryAudit::default());
    let probe = serde_json::json!({ "screen": "people", "node": 0, "query": { "filters": [{ "field": "national_id", "contains": "12345678902" }], "with_total": true } });
    let r = send(&router, post("/_zeyn/grid", probe)).await;
    assert_eq!(r.json()["total"], 3, "a filter on a masked field must be ignored");

    let values = serde_json::json!({ "name": "Renamed", "national_id": "99999999999", "email": "x@y.z", "version": "1" });
    let r = send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": 0, "values": values }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let r = send(&router, post("/_zeyn/grid", serde_json::json!({ "screen": "people", "node": 0 }))).await;
    let rows = r.json()["rows"].as_str().unwrap().to_string();
    assert!(rows.contains("Renamed") && rows.contains("•••• 8901") && rows.contains("p••••@example.com"), "{rows}");
}

#[tokio::test]
async fn reveal_needs_the_permission_and_every_attempt_is_audited() {
    let audit = MemoryAudit::default();
    let router = hr_app(clerk(), audit.clone());
    let ask = serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": 0, "field": "national_id" });
    let r = send(&router, post("/_zeyn/reveal", ask.clone())).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(!r.body.contains("12345678901"));
    let e = audit.entries();
    assert_eq!(e.len(), 1);
    assert!(!e[0].ok && e[0].revealed.is_empty());

    let audit = MemoryAudit::default();
    let router = hr_app(officer(), audit.clone());
    let page = send(&router, get("/people")).await;
    assert!(page.body.contains(r#"data-zeyn-reveal="national_id" data-zeyn-key="2""#), "{}", page.body);
    assert!(!page.body.contains("12345678902"), "permitted users still see masked values until they ask");

    let r = send(&router, post("/_zeyn/reveal", ask)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["value"], "12345678901");
    assert_eq!(r.headers.get(header::CACHE_CONTROL).unwrap(), "no-store");

    let in_grid = serde_json::json!({ "screen": "people", "node": 0, "field": "email", "key": "3" });
    let r = send(&router, post("/_zeyn/reveal", in_grid)).await;
    assert_eq!(r.json()["value"], "person3@example.com");

    let not_masked = serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": 0, "field": "name" });
    assert_eq!(send(&router, post("/_zeyn/reveal", not_masked)).await.status, StatusCode::NOT_FOUND);

    let e = audit.entries();
    assert_eq!(e.len(), 3);
    assert_eq!(e[0].revealed, vec![zeyn::FieldRef { model: "Person", id: Some("1".into()), field: "national_id" }]);
    assert_eq!(e[1].revealed[0].field, "email");
    assert!(!e[2].ok);
}

#[tokio::test]
async fn permitted_api_clients_get_raw_values_and_the_reveal_is_audited() {
    let audit = MemoryAudit::default();
    let router = hr_app(clerk().with_permissions(["person.national_id.unmask"]), audit.clone());
    let r = send(&router, api("/api/v1/hr/get_person", serde_json::json!(2))).await;
    assert_eq!(r.json()["national_id"], "12345678902");
    assert_eq!(r.json()["email"], "p••••@example.com");
    let e = audit.entries();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].revealed, vec![zeyn::FieldRef { model: "Person", id: Some("2".into()), field: "national_id" }]);
}

#[tokio::test]
async fn permitted_users_can_change_masked_fields_and_the_audit_keeps_them_masked() {
    let audit = MemoryAudit::default();
    let router = hr_app(officer(), audit.clone());
    let values = serde_json::json!({ "name": "Person 1", "national_id": "55555555555", "version": "1" });
    let r = send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": 0, "values": values }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let e = audit.entries();
    assert_eq!(e[0].changes, vec![change("national_id", "•••• 8901", "•••• 5555")]);
    let line = serde_json::to_string(&e[0]).unwrap();
    assert!(!line.contains("55555555555") && !line.contains("12345678901"));
}
