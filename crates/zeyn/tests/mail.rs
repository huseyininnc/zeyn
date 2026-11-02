//! E-mail notifications: a workflow move queues a localized message, the mail job writes it with
//! the file transport, failures stay queued for the next try, and configuration is checked.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Config, I18n, Identity, Mail, Mailer, StaticSource, TransitionRequest, Transitions, Workflow};

/// Stage of a request.
#[choice]
pub enum Stage {
    Draft,
    Submitted,
}

/// A request that moves through a workflow.
#[entity(table = "requests")]
pub struct Req {
    #[key]
    pub id: ReqId,
    pub title: String,
    pub owner: String,
    pub stage: Stage,
}

#[derive(Clone)]
pub struct Reqs(Arc<RwLock<Vec<Req>>>);

impl Service for Reqs {}

fn flow() -> Workflow<Stage> {
    Workflow::new().transition("submit", Stage::Draft, Stage::Submitted, "req.submit")
}

#[action(permission = "req.read")]
pub async fn move_req(r: TransitionRequest<ReqId>, store: Reqs, mailer: Mailer, ctx: Ctx) -> Result<Stage> {
    let (stage, owner, title) = {
        let mut rows = store.0.write().unwrap();
        let row = rows.iter_mut().find(|x| x.id == r.id).ok_or(Error::NotFound)?;
        row.stage = flow().apply(&ctx, row.stage, &r.transition, r.comment.as_deref())?;
        (row.stage, row.owner.clone(), row.title.clone())
    };
    mailer.send(&ctx, Mail::transition(&ctx, &owner, &title)?.locale("tr"))?;
    Ok(stage)
}

/// Which request a screen shows.
#[params]
pub struct ReqParams {
    pub id: i64,
}

#[screen(permission = "req.read")]
pub fn req_view(params: ReqParams, store: Reqs) -> Result<Screen> {
    let row = store.0.read().unwrap().iter().find(|x| x.id.0 == params.id).cloned().ok_or(Error::NotFound)?;
    Ok(Screen::window("Request").body(Transitions::of(flow(), row.stage, row.id, move_req)))
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("zeyn-mail-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn app(mailer: Mailer) -> Router {
    let identity = Identity::user("ali").with_permissions(["req.read", "req.submit"]);
    let rows = vec![Req {
        id: ReqId(1),
        title: "Laptop <b>& bag</b>".into(),
        owner: "Ayşe <ayse@example.com>".into(),
        stage: Stage::Draft,
    }];
    App::new("Test")
        .service(Reqs(Arc::new(RwLock::new(rows))))
        .i18n(I18n::new("en").catalog("tr", r#"{ "Stage": { "Submitted": "Gönderildi" } }"#).unwrap())
        .mailer(mailer)
        .auth(move |h: &HeaderMap| h.contains_key("x-user").then(|| identity.clone()))
        .feature(Feature::new("req").screen(req_view))
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn submit(router: &Router, comment: &str) -> StatusCode {
    let body = serde_json::json!({ "screen": "req_view", "params": { "id": 1 }, "node": 0, "values": { "transition": "submit", "comment": comment } });
    let req = Request::post("/_zeyn/form")
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let _ = res.into_body().collect().await;
    status
}

async fn wait_for_file(d: &std::path::Path) -> String {
    for _ in 0..200 {
        if let Some(e) =
            std::fs::read_dir(d).unwrap().flatten().find(|e| e.path().extension().is_some_and(|x| x == "eml"))
        {
            return std::fs::read_to_string(e.path()).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no mail was written to {}", d.display());
}

fn decoded(eml: &str) -> String {
    let joined = eml.replace("=\r\n", "");
    let mut out = String::new();
    let bytes = joined.as_bytes();
    let mut i = 0;
    let mut buf = Vec::new();
    while i < bytes.len() {
        if bytes[i] == b'='
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            buf.push(u8::from_str_radix(&joined[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            buf.push(bytes[i]);
            i += 1;
        }
    }
    out.push_str(&String::from_utf8_lossy(&buf));
    out
}

#[tokio::test]
async fn a_workflow_move_sends_a_localized_mail_from_the_job() {
    let d = dir("move");
    let router = app(Mailer::to_files("Zeyn <no-reply@example.com>", &d).unwrap());
    assert_eq!(submit(&router, "please <approve>").await, StatusCode::OK);
    let eml = wait_for_file(&d).await;
    assert!(eml.contains("To: =?utf-8?") || eml.contains("To: \"Ay"), "{eml}");
    assert!(eml.contains("ayse@example.com"));
    assert!(eml.contains("From: Zeyn <no-reply@example.com>"));
    assert!(eml.contains("multipart/alternative") && eml.contains("text/plain") && eml.contains("text/html"));
    let body = decoded(&eml);
    assert!(
        body.contains("Laptop <b>& bag</b>, ali tarafından Draft durumundan Submitted durumuna taşındı (submit)."),
        "{body}"
    );
    assert!(body.contains("<strong>Laptop &lt;b&gt;&amp; bag&lt;/b&gt;</strong>"), "{body}");
    assert!(body.contains("please &lt;approve&gt;"), "HTML gets escaped data: {body}");
    assert!(!body.contains("<b>& bag</b></strong>"));
    let _ = std::fs::remove_dir_all(&d);
}

#[tokio::test]
async fn sending_never_waits_for_the_server_and_failures_stay_queued() {
    let config = Config::load(vec![Box::new(StaticSource(
        [
            ("mail.from", "a@example.com"),
            ("mail.transport", "smtp"),
            ("mail.smtp.host", "127.0.0.1"),
            ("mail.smtp.port", "1"),
            ("mail.smtp.tls", "none"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    ))])
    .await
    .unwrap();
    let mailer = Mailer::from_config(&config).unwrap().retry(0, Duration::from_millis(10));
    let ctx = Ctx::new(Identity::user("x"), "en", Default::default(), Arc::new(I18n::new("en")));
    let started = std::time::Instant::now();
    mailer.send(&ctx, Mail::to("b@example.com").template("t")).unwrap();
    assert!(started.elapsed() < Duration::from_millis(200));
    assert_eq!(mailer.queued(), 1);
    assert!(mailer.flush().await.is_err(), "nothing listens on port 1");
    assert_eq!(mailer.queued(), 1, "the message waits for the next try");
    for _ in 0..zeyn::MAX_ATTEMPTS {
        let _ = mailer.flush().await;
    }
    assert_eq!(mailer.queued(), 0, "dropped after the last attempt");
    assert!(mailer.send(&ctx, Mail::to("not an address").template("t")).is_err());
    assert!(mailer.send(&ctx, Mail::to("x@example.com\r\nBcc: evil@example.com").template("t")).is_err());
}

#[tokio::test]
async fn configuration_is_checked() {
    let load = |pairs: &[(&str, &str)]| {
        let pairs: std::collections::HashMap<String, String> =
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        async move { Config::load(vec![Box::new(StaticSource(pairs))]).await.unwrap() }
    };
    assert!(Mailer::from_config(&load(&[]).await).is_err(), "mail.from is required");
    assert!(Mailer::from_config(&load(&[("mail.from", "nope")]).await).is_err());
    assert!(Mailer::from_config(&load(&[("mail.from", "a@b.co")]).await).is_ok(), "log transport by default");
    assert!(
        Mailer::from_config(&load(&[("mail.from", "a@b.co"), ("mail.transport", "file")]).await).is_err(),
        "file needs a directory"
    );
    assert!(Mailer::from_config(&load(&[("mail.from", "a@b.co"), ("mail.transport", "pigeon")]).await).is_err());
    let smtp = [
        ("mail.from", "a@b.co"),
        ("mail.transport", "smtp"),
        ("mail.smtp.host", "smtp.example.com"),
        ("mail.smtp.user", "u"),
    ];
    assert!(Mailer::from_config(&load(&smtp).await).is_err(), "a user needs mail.smtp.password");
    let mut full = smtp.to_vec();
    full.extend([("mail.smtp.password", "p"), ("mail.smtp.port", "587"), ("mail.smtp.tls", "starttls")]);
    assert!(Mailer::from_config(&load(&full).await).is_ok());
    full.push(("mail.smtp.tls", "maybe"));
    assert!(Mailer::from_config(&load(&full).await).is_err());
}
