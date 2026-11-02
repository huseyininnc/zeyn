//! Display components, the confirm effect, action buttons and stepped forms.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Identity, Render};

#[model]
pub struct Person {
    #[rule(required, max_len = 40)]
    pub name: String,
    #[rule(required, email)]
    pub email: String,
    #[rule(required)]
    pub city: String,
    pub born: Option<Date>,
}

#[derive(Clone, Default)]
pub struct Archive(Arc<Mutex<Vec<i64>>>);
impl Service for Archive {}

#[action(permission = "people.write")]
pub async fn archive_person(id: i64, archive: Archive) -> Result<i64> {
    archive.0.lock().unwrap().push(id);
    Ok(id)
}

#[action(permission = "people.write")]
pub async fn save_person(p: Valid<Person>) -> Result<String> {
    Ok(p.name.clone())
}

#[params]
pub struct PersonParams {
    pub id: i64,
}

#[screen(route = "/person", permission = "people.read")]
pub fn person(params: PersonParams) -> Screen {
    let p = Person { name: "Ada <b>".into(), email: "ada@example.com".into(), city: "İzmir".into(), born: Date::new(1990, 5, 1) };
    Screen::new("Person")
        .body(Breadcrumb::new().link("People", person, PersonParams { id: 0 }).current("Ada"))
        .body(Alert::warning("Check <the> address").title("Address").dismissible())
        .body(DescriptionList::of(&p).field(Person::name).field(Person::born).field(Person::city).item("Note", "").columns(3))
        .body(
            ActionButton::new("Archive", archive_person, params.id)
                .confirm("Archive this person?")
                .tone(Tone::Danger)
                .on_success(|_, ui| ui.effect(Effect::confirm("Open the list?", Effect::CloseAndRefresh))),
        )
        .body(
            Form::of(Person::default())
                .step("Who")
                .field(Person::name)
                .field(Person::email)
                .step("Where")
                .field(Person::city)
                .field(Person::born)
                .submit("Save", save_person),
        )
}

fn html() -> String {
    person::run(PersonParams { id: 7 }).html(&person::META)
}

#[test]
fn display_components_render_accessible_escaped_markup() {
    let h = html();
    assert!(h.contains(r#"<nav class="z-breadcrumb" aria-label="Breadcrumb"><ol><li><a href="/_zeyn/route/person?params=%7B%22id%22%3A0%7D" data-zeyn-base-href>People</a></li><li><span aria-current="page">Ada</span></li></ol></nav>"#));
    assert!(h.contains(r#"<span class="z-sr-only">Warning: </span>Address</p><p>Check &lt;the&gt; address</p>"#), "tone is spoken, not only colored");
    assert!(h.contains(r#"data-zeyn-dismiss aria-label="Dismiss""#));
    assert!(h.contains(r#"<dl class="z-details" style="--z-columns:3">"#));
    assert!(h.contains("<dd>Ada &lt;b&gt;</dd>"));
    assert!(h.contains(r#"<dd><time datetime="1990-05-01">5/1/1990</time></dd>"#));
    assert!(h.contains(r#"<dt>Note</dt><dd><span class="z-muted" aria-label="-">"#), "empty values show a dash");
    let mut out = String::new();
    let r = Render::new("x", "tr", I18n::builtin());
    Progress::new("Import", 30, 120).render(&mut out, &r);
    assert!(out.contains(r#"role="progressbar" aria-labelledby="x-progress-" aria-valuemin="0" aria-valuemax="120" aria-valuenow="30" aria-valuetext="25%""#));
    out.clear();
    Skeleton::lines(2).render(&mut out, &r);
    assert!(out.contains(r#"aria-busy="true"><span class="z-sr-only">Yükleniyor…</span>"#) && out.matches("z-skeleton-line").count() == 2);
    out.clear();
    Badge::new("Paid").tone(Tone::Success).render(&mut out, &r);
    assert_eq!(out, r#"<span class="z-badge z-tone-success">Paid</span>"#);
    out.clear();
    Timeline::new().event(Date::new(2024, 1, 5).unwrap(), "Created", None, Tone::Info).render(&mut out, &r);
    assert!(out.contains(r#"<time datetime="2024-01-05">05.01.2024</time>"#));
}

#[test]
fn action_buttons_are_forms_that_confirm_first_and_steps_render_one_fieldset_each() {
    let h = html();
    assert!(h.contains(r#"<form class="z-action" data-zeyn-submit="archive_person" data-zeyn-confirm="Archive this person?" data-text-title="Please confirm" data-text-ok="Archive" data-text-cancel="Cancel" novalidate><button type="submit" class="z-button z-danger-button""#));
    assert!(h.contains(r#"<ol class="z-steps" data-zeyn-steps aria-label="Steps""#));
    assert!(h.contains(r#"<fieldset class="z-step" data-zeyn-step="0"><legend class="z-step-legend">Who</legend>"#));
    assert!(h.contains(r#"<fieldset class="z-step" data-zeyn-step="1"><legend class="z-step-legend">Where</legend>"#));
    assert!(h.contains(r#"data-zeyn-step-next hidden"#), "step buttons appear only with JavaScript; without it every step shows");
    let schema = person::run(PersonParams { id: 7 }).schema(&person::META);
    assert_eq!(schema["body"][3]["type"], "action_button");
    assert_eq!(schema["body"][3]["confirm"], "Archive this person?");
    assert_eq!(schema["body"][4]["fields"][0]["step"], "Who");
}

#[test]
fn confirm_effects_carry_localized_texts_and_the_next_effect() {
    let e = Effect::confirm("Delete?", Effect::Toast { message: "Done".into() });
    let json = e.to_json(&Render::new("x", "tr", I18n::builtin()));
    assert_eq!(json["effect"], "confirm");
    assert_eq!(json["message"], "Delete?");
    assert_eq!(json["ok"], "Onayla");
    assert_eq!(json["cancel"], "Vazgeç");
    assert_eq!(json["then"], serde_json::json!({ "effect": "toast", "message": "Done" }));
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app(archive: Archive) -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(archive)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("people").screen(person))
        .router()
        .unwrap()
}

async fn call(router: &axum::Router, path: &str, perms: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let req = Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn action_buttons_run_with_the_servers_input_and_the_actions_permission() {
    let archive = Archive::default();
    let router = app(archive.clone());
    let body = serde_json::json!({ "screen": "person", "params": { "id": 7 }, "node": "3", "values": { "id": 999 } });
    let (status, _) = call(&router, "/_zeyn/form", "people.read", body.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, json) = call(&router, "/_zeyn/form", "people.read,people.write", body).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(*archive.0.lock().unwrap(), vec![7], "values from the client are ignored");
    assert_eq!(json["effects"][0]["effect"], "confirm");
    assert_eq!(json["effects"][0]["then"]["effect"], "close_and_refresh");
}

#[tokio::test]
async fn step_checks_report_only_the_steps_fields_and_need_the_save_permission() {
    let router = app(Archive::default());
    let step = |n: u64, values: serde_json::Value| serde_json::json!({ "screen": "person", "params": { "id": 1 }, "node": "4", "key": "_step", "input": { "step": n, "values": values } });
    let (status, json) = call(&router, "/_zeyn/data", "people.read,people.write", step(0, serde_json::json!({ "name": "", "email": "nope" }))).await;
    assert_eq!(status, StatusCode::OK);
    let fields: Vec<&str> = json["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["name", "email"], "city belongs to the next step and is not reported");
    let (_, json) = call(&router, "/_zeyn/data", "people.read,people.write", step(0, serde_json::json!({ "name": "Ada", "email": "ada@example.com" }))).await;
    assert_eq!(json["errors"], serde_json::json!([]));
    let (status, _) = call(&router, "/_zeyn/data", "people.read,people.write", step(5, serde_json::json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&router, "/_zeyn/data", "people.read", step(0, serde_json::json!({}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "checking a step needs the save action's permission");
}
