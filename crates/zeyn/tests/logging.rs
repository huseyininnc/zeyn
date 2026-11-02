//! What reaches the logs. Kept in its own test binary: a `tracing` subscriber set for one thread
//! misses events when other tests run framework code on other threads at the same time.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Identity, MemoryAudit};

#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    #[rule(required)]
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email)]
    pub email: Option<String>,
    #[version]
    pub version: i64,
}

#[derive(Clone)]
pub struct People(Arc<RwLock<Vec<Person>>>);

impl Service for People {}

impl People {
    fn seeded() -> Self {
        let row = |i: i64| Person {
            id: PersonId(i),
            name: format!("Person {i}"),
            national_id: format!("1234567890{i}"),
            email: Some(format!("person{i}@example.com")),
            version: 1,
        };
        People(Arc::new(RwLock::new((1..=3).map(row).collect())))
    }

    fn get(&self, id: i64) -> Option<Person> {
        self.0.read().unwrap().iter().find(|c| c.id.0 == id).cloned()
    }
}

#[action(permission = "hr.read")]
pub async fn find_people(q: DataQuery, store: People) -> Result<Page<Person>> {
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
    *current = p.into_inner();
    current.version += 1;
    Ok(current.clone())
}

#[params]
pub struct PersonParams {
    pub id: i64,
}

#[screen(route = "/people", permission = "hr.read")]
pub fn people() -> Screen {
    Screen::new("People").body(DataGrid::of(find_people).column(Person::name).column(Person::national_id).column(Person::email))
}

#[screen(permission = "hr.read")]
pub fn person(params: PersonParams, store: People) -> Result<Screen> {
    let p = store.get(params.id).ok_or(Error::NotFound)?;
    Ok(Screen::window("Person").body(Form::of(p).field(Person::name).field(Person::national_id).field(Person::email).submit("Save", save_person)))
}

fn app() -> Router {
    let identity = Identity::user("clerk").with_permissions(["hr.read", "hr.write", "person.national_id.unmask", "person.email.unmask"]);
    App::new("Test")
        .service(People::seeded())
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .interceptor(AuditLog(MemoryAudit::default()))
        .feature(Feature::new("hr").screen(people).screen(person).api(get_person).api(save_person))
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

fn post(path: &str, body: serde_json::Value, api: bool) -> Request<Body> {
    let b = Request::post(path).header("x-user", "1").header(header::CONTENT_TYPE, "application/json");
    let b = if api {
        b.header(header::AUTHORIZATION, "Bearer t")
    } else {
        b.header(header::COOKIE, format!("zeyn_csrf={TOKEN}")).header("x-zeyn-csrf", TOKEN)
    };
    b.body(Body::from(body.to_string())).unwrap()
}

/// Collects everything a `tracing` subscriber writes.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn masked_values_and_input_never_reach_the_logs() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);
    let router = app();

    let get = Request::get("/people?q=12345678901").header("x-user", "1").body(Body::empty()).unwrap();
    assert_eq!(send(&router, get).await.0, StatusCode::OK);
    let values = serde_json::json!({ "name": "Person 1", "national_id": "55555555555", "version": "1" });
    let form = serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": 0, "values": values });
    let (status, body) = send(&router, post("/_zeyn/form", form, false)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = send(&router, post("/api/v1/hr/get_person", serde_json::json!(2), true)).await;
    assert!(status == StatusCode::OK && body.contains("12345678902"), "{body}");
    let reveal = serde_json::json!({ "screen": "person", "params": { "id": 3 }, "node": 0, "field": "national_id" });
    let (status, body) = send(&router, post("/_zeyn/reveal", reveal, false)).await;
    assert!(status == StatusCode::OK && body.contains("12345678903"), "{body}");
    let (status, _) = send(&router, post("/api/v1/hr/save_person", serde_json::json!({ "id": 9, "name": "Ghost", "national_id": "99999999999", "version": 1 }), true)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for expected in ["request", "action=\"save_person\"", "user=\"clerk\"", "status=200", "status=404", "interceptor", "run", "ms="] {
        assert!(log.contains(expected), "`{expected}` is missing from the log:\n{log}");
    }
    for secret in ["55555555555", "12345678901", "12345678902", "12345678903", "99999999999", "person1@example.com", "person2@example.com"] {
        assert!(!log.contains(secret), "`{secret}` reached the log:\n{log}");
    }
}
