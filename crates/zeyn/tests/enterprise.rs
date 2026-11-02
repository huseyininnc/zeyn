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
        let mut rows: Vec<Person> = (1..=3)
            .map(|i| Person {
                id: PersonId(i),
                name: format!("Person {i}"),
                national_id: format!("1234567890{i}"),
                email: Some(format!("person{i}@example.com")),
                version: 1,
            })
            .collect();
        rows.push(Person { id: PersonId(4), name: "New person".into(), national_id: String::new(), email: None, version: 1 });
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
    assert_eq!(r.json()["total"], 4, "a filter on a masked field must be ignored");

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

#[entity(table = "cases")]
#[sensitive]
pub struct Case {
    #[key]
    pub id: CaseId,
    pub subject: String,
}

#[derive(Clone)]
pub struct Cases(Vec<Case>);

impl Service for Cases {}

#[action(permission = "cases.read")]
pub async fn find_cases(q: GridQuery, store: Cases) -> Result<Page<Case>> {
    Ok(q.apply(store.0.clone()))
}

#[action(permission = "cases.read")]
pub async fn get_case(id: i64, store: Cases) -> Result<Case> {
    store.0.iter().find(|c| c.id.0 == id).cloned().ok_or(Error::NotFound)
}

#[screen(route = "/cases", permission = "cases.read")]
pub fn cases() -> Screen {
    Screen::new("Cases").body(Grid::of(find_cases).column(Case::subject))
}

#[screen(permission = "cases.read")]
pub fn case(params: CustomerParams, store: Cases) -> Result<Screen> {
    let c = store.0.iter().find(|c| c.id.0 == params.id).cloned().ok_or(Error::NotFound)?;
    Ok(Screen::window("Case").body(Form::of(c).field(Case::subject)))
}

#[screen(route = "/cases/open", permission = "cases.read", reason)]
pub fn case_file() -> Screen {
    Screen::new("Case file").body(Grid::of(find_cases).column(Case::subject))
}

#[screen(route = "/cases/other", permission = "cases.read", reason)]
pub fn other_file() -> Screen {
    Screen::new("Other file")
}

#[screen(route = "/overview", permission = "cases.read")]
pub fn overview() -> Screen {
    Screen::new("Overview")
}

fn cases_app(identity: Identity, audit: MemoryAudit) -> Router {
    let store = Cases((1..=2).map(|i| Case { id: CaseId(i), subject: format!("Subject {i}") }).collect());
    App::new("Test")
        .service(store)
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .feature(
            Feature::new("cases")
                .screen(cases)
                .screen(case)
                .screen(case_file)
                .screen(other_file)
                .screen(overview)
                .api(get_case),
        )
        .router()
        .unwrap()
}

fn case_reader() -> Identity {
    Identity::user("nurse").with_permissions(["cases.read"])
}

fn with_cookie(mut req: Request<Body>, cookie: &str) -> Request<Body> {
    let existing = req.headers().get(header::COOKIE).map(|v| v.to_str().unwrap().to_string());
    let value = match existing {
        Some(e) => format!("{e}; {cookie}"),
        None => cookie.to_string(),
    };
    req.headers_mut().insert(header::COOKIE, value.parse().unwrap());
    req
}

#[tokio::test]
async fn showing_a_sensitive_record_writes_a_read_record() {
    let audit = MemoryAudit::default();
    let router = cases_app(case_reader(), audit.clone());
    assert_eq!(send(&router, get("/overview")).await.status, StatusCode::OK);
    assert!(audit.entries().is_empty(), "screens without personal data are not audited");

    send(&router, get("/cases")).await;
    send(&router, get("/_zeyn/screen/case?params=%7B%22id%22%3A2%7D")).await;
    send(&router, api("/api/v1/cases/get_case", serde_json::json!(1))).await;
    let e = audit.entries();
    assert_eq!(e.len(), 3);
    let ids = |i: usize| e[i].reads.iter().map(|r| (r.model, r.id.clone().unwrap())).collect::<Vec<_>>();
    assert_eq!(ids(0), [("Case", "1".to_string()), ("Case", "2".to_string())]);
    assert_eq!(e[0].user.as_deref(), Some("nurse"));
    assert_eq!(ids(1), [("Case", "2".to_string())]);
    assert_eq!(ids(2), [("Case", "1".to_string())]);
    assert!(e.iter().all(|x| x.at_unix_ms > 0));
}

#[tokio::test]
async fn a_screen_that_asks_for_a_reason_shows_no_data_until_one_is_given() {
    let audit = MemoryAudit::default();
    let router = cases_app(case_reader(), audit.clone());
    let r = send(&router, get("/cases/open")).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED);
    assert!(r.body.contains(r#"data-zeyn-reason="case_file""#) && !r.body.contains("Subject 1"), "{}", r.body);
    let r = send(&router, get("/_zeyn/screen/case_file")).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(r.json()["error"], "reason_required");
    let r = send(&router, post("/_zeyn/grid", serde_json::json!({ "screen": "case_file", "node": 0 }))).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED);
    assert!(!r.body.contains("Subject"));

    let r = send(&router, post("/_zeyn/reason", serde_json::json!({ "screen": "case_file", "reason": "x" }))).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = send(&router, post("/_zeyn/reason", serde_json::json!({ "screen": "overview", "reason": "Checking" }))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    let r = send(&router, post("/_zeyn/reason", serde_json::json!({ "screen": "case_file", "reason": "Hasta çağrısı #42" }))).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let cookie = r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains("Max-Age=900"), "{cookie}");
    let pair = cookie.split(';').next().unwrap().to_string();

    let r = send(&router, with_cookie(get("/cases/open"), &pair)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.contains("Subject 1"));
    let r = send(&router, with_cookie(get("/cases/other"), &pair)).await;
    assert_eq!(r.status, StatusCode::PRECONDITION_REQUIRED, "a reason only opens the screen it was given for");

    let e = audit.entries();
    let opened = e.iter().find(|x| x.ok && x.screen == "case_file").unwrap();
    assert_eq!(opened.reason.as_deref(), Some("Hasta çağrısı #42"));
    assert!(opened.reads.iter().any(|r| r.model == "case_file"));
    assert!(opened.reads.iter().any(|r| r.model == "Case" && r.id.as_deref() == Some("2")));
}

#[tokio::test]
async fn reasons_need_access_to_the_screen() {
    let router = cases_app(Identity::user("guest"), MemoryAudit::default());
    let r = send(&router, post("/_zeyn/reason", serde_json::json!({ "screen": "case_file", "reason": "Curious" }))).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(r.headers.get(header::SET_COOKIE).is_none());
}

#[choice]
pub enum Stage {
    Draft,
    Submitted,
    Approved,
    Rejected,
}

#[entity(table = "requests")]
pub struct Request2 {
    #[key]
    pub id: Request2Id,
    pub title: String,
    pub stage: Stage,
}

fn request_flow() -> Workflow<Stage> {
    Workflow::new()
        .transition("submit", Stage::Draft, Stage::Submitted, "req.submit")
        .transition("approve", Stage::Submitted, Stage::Approved, "req.approve")
        .transition("reject", Stage::Submitted, Stage::Rejected, "req.approve")
}

#[derive(Clone)]
pub struct Requests(Arc<RwLock<Vec<Request2>>>);

impl Service for Requests {}

#[action(permission = "req.read", audit)]
pub async fn move_request(r: TransitionRequest<Request2Id>, store: Requests, ctx: Ctx) -> Result<Stage> {
    let mut rows = store.0.write().unwrap();
    let row = rows.iter_mut().find(|x| x.id == r.id).ok_or(Error::NotFound)?;
    row.stage = request_flow().apply(&ctx, row.stage, &r.transition, r.comment.as_deref())?;
    Ok(row.stage)
}

#[screen(permission = "req.read")]
pub fn request_view(params: CustomerParams, store: Requests) -> Result<Screen> {
    let row = store.0.read().unwrap().iter().find(|x| x.id.0 == params.id).cloned().ok_or(Error::NotFound)?;
    Ok(Screen::window("Request").body(Transitions::of(request_flow(), row.stage, row.id, move_request)))
}

fn requests_app(identity: Identity, audit: MemoryAudit, store: Requests) -> Router {
    App::new("Test")
        .service(store)
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(audit))
        .feature(Feature::new("req").screen(request_view))
        .router()
        .unwrap()
}

fn requests() -> Requests {
    let rows = vec![
        Request2 { id: Request2Id(1), title: "Draft one".into(), stage: Stage::Draft },
        Request2 { id: Request2Id(2), title: "Waiting".into(), stage: Stage::Submitted },
    ];
    Requests(Arc::new(RwLock::new(rows)))
}

fn requester() -> Identity {
    Identity::user("ali").with_permissions(["req.read", "req.submit"])
}

fn approver() -> Identity {
    Identity::user("mehmet").with_permissions(["req.read", "req.approve"])
}

fn move_to(id: i64, transition: &str, comment: &str) -> Request<Body> {
    post(
        "/_zeyn/form",
        serde_json::json!({ "screen": "request_view", "params": { "id": id }, "node": 0, "values": { "transition": transition, "comment": comment } }),
    )
}

#[tokio::test]
async fn only_the_moves_a_user_may_make_from_the_current_state_are_offered() {
    let store = requests();
    let view = |identity: Identity, id: u8| {
        let router = requests_app(identity, MemoryAudit::default(), store.clone());
        async move { send(&router, get(&format!("/_zeyn/screen/request_view?params=%7B%22id%22%3A{id}%7D"))).await.body }
    };
    let html = view(requester(), 1).await;
    assert!(html.contains(r#"data-zeyn-transition="submit""#) && !html.contains("approve"), "{html}");
    let html = view(approver(), 1).await;
    assert!(!html.contains("data-zeyn-transition"), "{html}");
    let html = view(approver(), 2).await;
    assert!(html.contains(r#"data-zeyn-transition="approve""#) && html.contains(r#"data-zeyn-transition="reject""#));
    assert!(html.contains("<strong>Submitted</strong>"));
}

#[tokio::test]
async fn moves_are_checked_on_the_server_and_audited() {
    let store = requests();
    let audit = MemoryAudit::default();
    let ali = requests_app(requester(), audit.clone(), store.clone());
    let mehmet = requests_app(approver(), audit.clone(), store.clone());

    assert_eq!(send(&ali, move_to(2, "approve", "")).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&mehmet, move_to(1, "approve", "")).await.status, StatusCode::CONFLICT);
    assert_eq!(send(&mehmet, move_to(2, "archive", "")).await.status, StatusCode::NOT_FOUND);

    let r = send(&ali, move_to(1, "submit", "  Please approve  ")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.json()["effects"][0]["message"], "Done.");
    let r = send(&mehmet, move_to(1, "approve", "OK")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(send(&mehmet, move_to(1, "reject", "")).await.status, StatusCode::CONFLICT, "an approved request cannot be rejected");
    assert_eq!(store.0.read().unwrap()[0].stage, Stage::Approved);

    let done: Vec<_> = audit.entries().into_iter().filter(|e| e.ok).collect();
    assert_eq!(done.len(), 2);
    let t = done[0].transition.as_ref().unwrap();
    assert_eq!((t.name, t.from.as_str(), t.to.as_str(), t.comment.as_deref()), ("submit", "Draft", "Submitted", Some("Please approve")));
    assert_eq!(done[0].user.as_deref(), Some("ali"));
    assert_eq!(done[1].transition.as_ref().unwrap().to, "Approved");
    assert_eq!(done[1].user.as_deref(), Some("mehmet"));
    assert!(audit.entries().iter().filter(|e| !e.ok).all(|e| e.transition.is_none()));
}

#[tokio::test]
async fn long_comments_are_refused() {
    let router = requests_app(requester(), MemoryAudit::default(), requests());
    let r = send(&router, move_to(1, "submit", &"x".repeat(1001))).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.json()["errors"][0]["field"], "comment");
}

#[test]
fn workflow_definitions_are_checked() {
    let bad = Workflow::new()
        .transition("go", Stage::Draft, Stage::Submitted, "Bad Permission")
        .transition("go", Stage::Draft, Stage::Approved, "req.go");
    let problems = bad.check().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("invalid permission")));
    assert!(problems.iter().any(|p| p.contains("declared twice")));
    assert!(request_flow().check().is_ok());
}

#[tokio::test]
async fn empty_masked_fields_can_be_filled_in_but_not_overwritten_later() {
    let router = hr_app(clerk(), MemoryAudit::default());
    let form = send(&router, get("/_zeyn/screen/person?params=%7B%22id%22%3A4%7D")).await;
    assert!(form.body.contains(r#"name="national_id""#) && !form.body.contains("data-zeyn-masked"), "{}", form.body);
    let values = serde_json::json!({ "name": "New person", "national_id": "98765432100", "email": "new@example.com", "version": "1" });
    let r = send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "person", "params": { "id": 4 }, "node": 0, "values": values }))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let form = send(&router, get("/_zeyn/screen/person?params=%7B%22id%22%3A4%7D")).await;
    assert!(form.body.contains(r#"value="•••• 2100""#) && !form.body.contains("98765432100"), "{}", form.body);
    let again = serde_json::json!({ "name": "New person", "national_id": "11111111111", "version": "2" });
    send(&router, post("/_zeyn/form", serde_json::json!({ "screen": "person", "params": { "id": 4 }, "node": 0, "values": again }))).await;
    let form = send(&router, get("/_zeyn/screen/person?params=%7B%22id%22%3A4%7D")).await;
    assert!(form.body.contains(r#"value="•••• 2100""#), "{}", form.body);
}

#[tokio::test]
async fn every_response_carries_the_build_id_that_the_page_was_built_with() {
    let router = crm_app(crm_writer(), MemoryAudit::default()).router().unwrap();
    let page = send(&router, get("/customers")).await;
    let id = page.headers.get("x-zeyn-build").unwrap().to_str().unwrap().to_string();
    assert_eq!(id, zeyn::server::build_id());
    assert!(!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)));
    assert!(page.body.contains(&format!(r#"<meta name="zeyn-build" content="{id}">"#)));
    assert!(page.body.contains("/_zeyn/enterprise.js"));
    let grid = send(&router, post("/_zeyn/grid", serde_json::json!({ "screen": "customers", "node": 0 }))).await;
    assert_eq!(grid.headers.get("x-zeyn-build").unwrap(), id.as_str());
    let missing = send(&router, get("/nope")).await;
    assert_eq!(missing.headers.get("x-zeyn-build").unwrap(), id.as_str());
    let js = send(&router, Request::get("/_zeyn/enterprise.js").body(Body::empty()).unwrap()).await;
    assert_eq!(js.status, StatusCode::OK);
    assert!(js.body.contains("x-zeyn-build"));
}

fn from(path: &str, ip: &str) -> Request<Body> {
    let mut req = Request::get(path).body(Body::empty()).unwrap();
    let addr: std::net::SocketAddr = format!("{ip}:5000").parse().unwrap();
    req.extensions_mut().insert(axum::extract::ConnectInfo(addr));
    req
}

#[tokio::test]
async fn health_is_always_up_and_ready_reports_failed_checks_by_name_only() {
    let db_up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = db_up.clone();
    let router = App::new("T")
        .base_path("/billing")
        .readiness("database", move || {
            let up = flag.load(std::sync::atomic::Ordering::SeqCst);
            async move { up }
        })
        .readiness("slow", || async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            true
        })
        .feature(Feature::new("x").screen(overview))
        .router()
        .unwrap();
    let r = send(&router, Request::get("/billing/_zeyn/health").body(Body::empty()).unwrap()).await;
    assert_eq!((r.status, r.body.as_str()), (StatusCode::OK, "ok"));
    assert_eq!(r.headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    let r = send(&router, Request::get("/billing/_zeyn/ready").body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(r.json(), serde_json::json!({ "status": "not_ready", "failed": ["database"] }));
    db_up.store(true, std::sync::atomic::Ordering::SeqCst);
    let r = send(&router, Request::get("/billing/_zeyn/ready").body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(send(&router, Request::get("/_zeyn/health").body(Body::empty()).unwrap()).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn probes_skip_rate_limits_but_not_ip_rules_unless_allowed() {
    let app = || {
        App::new("T")
            .rate_limit(RateLimit::per_ip(1, std::time::Duration::from_secs(60)))
            .allow_only_ips(["10.0.0.0/8"])
            .deny_ips(["10.9.9.9"])
            .feature(Feature::new("x").screen(overview))
    };
    let router = app().router().unwrap();
    for _ in 0..5 {
        assert_eq!(send(&router, from("/_zeyn/health", "10.1.1.1")).await.status, StatusCode::OK);
    }
    assert_eq!(send(&router, from("/_zeyn/health", "192.168.1.5")).await.status, StatusCode::FORBIDDEN);

    let router = app().probe_ips(["192.168.1.0/24", "10.9.9.9"]).router().unwrap();
    assert_eq!(send(&router, from("/_zeyn/ready", "192.168.1.5")).await.status, StatusCode::OK);
    assert_eq!(send(&router, from("/overview", "192.168.1.5")).await.status, StatusCode::FORBIDDEN, "probe addresses only reach the probes");
    assert_eq!(send(&router, from("/_zeyn/health", "10.9.9.9")).await.status, StatusCode::FORBIDDEN, "denied stays denied");
}

#[tokio::test]
async fn dev_latency_slows_calls_in_debug_builds_but_not_probes_or_assets() {
    let delay = std::time::Duration::from_millis(120);
    let router = App::new("T").dev_latency(delay).feature(Feature::new("x").screen(overview)).router().unwrap();
    let timed = |path: &'static str| {
        let router = router.clone();
        async move {
            let started = std::time::Instant::now();
            send(&router, Request::get(path).body(Body::empty()).unwrap()).await;
            started.elapsed()
        }
    };
    let call = timed("/overview").await;
    if cfg!(debug_assertions) {
        assert!(call >= delay, "{call:?}");
    } else {
        assert!(call < delay, "release builds never delay");
    }
    assert!(timed("/_zeyn/health").await < delay);
    assert!(timed("/_zeyn/zeyn.js").await < delay);
}

#[action(permission = "crm.write", reauth)]
pub async fn close_account(id: i64) -> Result<i64> {
    Ok(id)
}

#[screen(permission = "crm.write")]
pub fn closing(params: CustomerParams, store: Customers) -> Result<Screen> {
    let c = store.get(params.id).ok_or(Error::NotFound)?;
    Ok(Screen::window("Close").body(Form::of(c).field(Customer::name).submit("Close", save_after_reauth)))
}

#[action(permission = "crm.write", reauth)]
pub async fn save_after_reauth(c: Valid<Customer>) -> Result<i64> {
    Ok(c.id.0)
}

fn reauth_app(identity: Identity) -> Router {
    App::new("T")
        .service(Customers::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .login_url("https://sso.example.com/login")
        .idle_timeout(std::time::Duration::from_secs(900))
        .reauth_within(std::time::Duration::from_secs(300))
        .feature(Feature::new("crm").screen(customers).screen(closing).api(close_account))
        .router()
        .unwrap()
}

#[tokio::test]
async fn reauth_actions_need_a_recent_sign_in_reported_by_the_identity_provider() {
    let now = DateTime::now();
    let form = serde_json::json!({ "screen": "closing", "params": { "id": 1 }, "node": 0, "values": { "name": "X" } });
    for identity in [crm_writer(), crm_writer().with_authenticated_at(now.add_ms(-600_000)), crm_writer().with_authenticated_at(now.add_ms(600_000))] {
        let router = reauth_app(identity);
        let r = send(&router, post("/_zeyn/form", form.clone())).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
        assert_eq!(r.json()["error"], "reauth_required");
        let r = send(&router, api("/api/v1/crm/close_account", serde_json::json!(5))).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
        assert_eq!(r.json()["type"], "urn:zeyn:problem:reauth_required");
    }
    let router = reauth_app(crm_writer().with_authenticated_at(now.add_ms(-60_000)));
    assert_eq!(send(&router, post("/_zeyn/form", form)).await.status, StatusCode::OK);
    assert_eq!(send(&router, api("/api/v1/crm/close_account", serde_json::json!(5))).await.json(), 5);
}

#[tokio::test]
async fn forward_auth_reads_the_sign_in_time_and_dev_auth_is_always_fresh() {
    use zeyn::server::{Authenticator, ForwardAuth};
    let auth = ForwardAuth::new("x-user").auth_time("x-auth-time");
    let mut h = HeaderMap::new();
    h.insert("x-user", "ayse".parse().unwrap());
    h.insert("x-auth-time", "1700000000".parse().unwrap());
    assert_eq!(auth.authenticate(&h).unwrap().authenticated_at, Some(DateTime::from_unix_ms(1_700_000_000_000)));
    h.insert("x-auth-time", "2024-01-05T11:30:00Z".parse().unwrap());
    assert_eq!(auth.authenticate(&h).unwrap().authenticated_at, DateTime::parse("2024-01-05T11:30:00Z"));
    h.insert("x-auth-time", "yesterday".parse().unwrap());
    assert_eq!(auth.authenticate(&h).unwrap().authenticated_at, None);
    let dev = DevAuth::superuser("dev").authenticate(&HeaderMap::new()).unwrap();
    assert!(dev.authenticated_at.is_some());
}

#[tokio::test]
async fn the_page_tells_the_runtime_about_the_idle_timeout_and_keepalive_needs_csrf() {
    let router = reauth_app(crm_writer());
    let page = send(&router, get("/customers")).await;
    let start = page.body.find(r#"<meta name="zeyn-session" content=""#).expect("session meta");
    let meta = &page.body[start..start + 300];
    assert!(meta.contains("&quot;idle&quot;:900") && meta.contains("&quot;warn&quot;:60"), "{meta}");
    assert!(meta.contains("https://sso.example.com/login"));
    let r = send(&router, Request::post("/_zeyn/keepalive").header("x-user", "1").body(Body::empty()).unwrap()).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = send(&router, post("/_zeyn/keepalive", serde_json::json!({}))).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let anonymous = App::new("T").idle_timeout(std::time::Duration::from_secs(60)).feature(Feature::new("x").screen(overview)).router().unwrap();
    let r = send(&anonymous, Request::get("/overview").body(Body::empty()).unwrap()).await;
    assert!(!r.body.contains("zeyn-session"));
}
