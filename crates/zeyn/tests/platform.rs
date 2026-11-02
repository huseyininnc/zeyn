use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{
    Authenticator, BoxFuture, Config, ConfigSource, FixedLicense, ForwardAuth, HeaderTenant, Identity, JsonFileSource,
    License, LicenseSource, PerTenant, Services, StaticSource, SubdomainTenant, TenantResolver,
};

fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn secrets_never_print() {
    let s = Secret::new("Server=db;Password=hunter2".to_string());
    assert_eq!(format!("{s} {s:?}"), "[redacted] [redacted]");
    assert!(s.expose().contains("hunter2"));
}

#[tokio::test]
async fn config_sources_are_layered_and_report_their_origin() {
    let dir = std::env::temp_dir().join(format!("zeyn-config-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("app.json");
    std::fs::write(&file, r#"{ "Db": { "Url": "file-db", "Pool": 8 }, "feature": { "beta": true } }"#).unwrap();

    let config = Config::load(vec![
        Box::new(StaticSource(map(&[("db.url", "default"), ("app.name", "Demo")]))),
        Box::new(JsonFileSource(file.clone())),
    ])
    .await
    .unwrap();
    assert_eq!(config.get("db.url"), Some("file-db"));
    assert_eq!(config.get("db.pool"), Some("8"));
    assert_eq!(config.get("feature.beta"), Some("true"));
    assert_eq!(config.get("app.name"), Some("Demo"));
    assert_eq!(config.source_of("db.url"), Some("json file"));
    assert_eq!(format!("{:?}", config.secret("db.url").unwrap()), "[redacted]");
    let missing = config.require("db.password").unwrap_err().to_string();
    assert!(!missing.contains("file-db"));
    assert!(!format!("{config:?}").contains("file-db"));

    let err = Config::load(vec![Box::new(JsonFileSource(dir.join("nope.json")))]).await.unwrap_err();
    assert!(err.starts_with("config source `json file`"));
}

struct Vault(Arc<AtomicUsize>);

impl ConfigSource for Vault {
    fn name(&self) -> &str {
        "vault"
    }

    fn load(&self) -> BoxFuture<Result<HashMap<String, String>, String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(map(&[("tenants.acme.db_url", "postgres://acme"), ("tenants.globex.db_url", "postgres://globex")])) })
    }
}

#[test]
fn tenant_ids_are_strict() {
    assert!(Tenant::new("acme-01").is_some());
    for bad in ["", "Acme", "acme corp", "../etc", "a'; drop", &"x".repeat(64)] {
        assert!(Tenant::new(bad).is_none(), "{bad}");
    }
}

#[tokio::test]
async fn per_tenant_services_are_built_once_per_tenant() {
    let calls = Arc::new(AtomicUsize::new(0));
    let config = Config::load(vec![Box::new(Vault(Arc::new(AtomicUsize::new(0))))]).await.unwrap();
    let counter = calls.clone();
    let pools: PerTenant<String> = PerTenant::new(move |tenant, config| {
        counter.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(config.secret(&format!("tenants.{}.db_url", tenant.id()))?.expose().clone()) })
    });
    let ctx = |t: &str| Ctx::for_tests(Identity::anonymous(), Services::default()).with_config(config.clone()).with_tenant(Tenant::new(t));
    assert_eq!(pools.get(&ctx("acme")).await.unwrap(), "postgres://acme");
    assert_eq!(pools.get(&ctx("acme")).await.unwrap(), "postgres://acme");
    assert_eq!(pools.get(&ctx("globex")).await.unwrap(), "postgres://globex");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(pools.get(&ctx("initech")).await.is_err());
    let none = Ctx::for_tests(Identity::anonymous(), Services::default());
    assert!(matches!(pools.get(&none).await, Err(Error::Denied(_))));
}

#[test]
fn tenants_resolve_from_headers_and_subdomains() {
    let mut h = HeaderMap::new();
    h.insert("x-tenant", "Acme".parse().unwrap());
    h.insert(header::HOST, "globex.app.example.com:443".parse().unwrap());
    assert_eq!(HeaderTenant("x-tenant").resolve(&h), Some("acme".into()));
    assert_eq!(SubdomainTenant("app.example.com").resolve(&h), Some("globex".into()));
    h.insert(header::HOST, "a.b.app.example.com".parse().unwrap());
    assert_eq!(SubdomainTenant("app.example.com").resolve(&h), None);
    h.insert(header::HOST, "evil.com".parse().unwrap());
    assert_eq!(SubdomainTenant("app.example.com").resolve(&h), None);
}

#[test]
fn forward_auth_maps_groups_and_needs_the_proxy_secret() {
    let auth = ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .tenant("x-forwarded-tenant")
        .require_header("x-proxy-secret", "s3cret")
        .role("stock-admins", ["stock.read", "stock.write"])
        .role("viewers", ["stock.read"]);
    let mut h = HeaderMap::new();
    h.insert("x-forwarded-user", "ayse".parse().unwrap());
    h.insert("x-forwarded-groups", "viewers, other".parse().unwrap());
    h.insert("x-forwarded-tenant", "acme".parse().unwrap());
    assert_eq!(auth.authenticate(&h), None);
    h.insert("x-proxy-secret", "wrong".parse().unwrap());
    assert_eq!(auth.authenticate(&h), None);
    h.insert("x-proxy-secret", "s3cret".parse().unwrap());
    let id = auth.authenticate(&h).unwrap();
    assert_eq!(id.user.as_deref(), Some("ayse"));
    assert!(id.permissions.contains("stock.read") && !id.permissions.contains("stock.write"));
    assert_eq!(id.tenant.as_deref(), Some("acme"));
}

#[screen(route = "/pos", public)]
pub fn pos(_: (), ctx: Ctx) -> Screen {
    let who = ctx.license().map(|l| l.holder.clone()).unwrap_or_default();
    let tenant = ctx.tenant().map(|t| t.id().to_string()).unwrap_or_default();
    Screen::new(Text::from(format!("licensed to {who} tenant {tenant}")))
}

#[screen(route = "/secret", permission = "secret.read")]
pub fn secret_page() -> Screen {
    Screen::new("Secret")
}

fn license(until: (i32, u8, u8), modules: &[&str]) -> License {
    License {
        holder: "Acme".into(),
        valid_until: Date::new(until.0, until.1, until.2).unwrap(),
        modules: modules.iter().map(|m| m.to_string()).collect(),
        max_users: None,
    }
}

async fn get(router: &Router, path: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, String) {
    let mut b = Request::get(path);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let res = router.clone().oneshot(b.body(Body::empty()).unwrap()).await.unwrap();
    let (status, h) = (res.status(), res.headers().clone());
    (status, h, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

fn licensed_app(source: impl LicenseSource, module: &'static str) -> Router {
    App::new("POS")
        .fixed_now(DateTime::parse("2026-11-02T10:00:00Z").unwrap())
        .license(source, Duration::from_secs(60))
        .tenancy(HeaderTenant("x-tenant"))
        .feature(Feature::new("pos").guard(Licensed::module(module)).screen(pos))
        .router()
        .unwrap()
}

#[tokio::test]
async fn licenses_open_modules_until_they_expire() {
    let r = licensed_app(FixedLicense(license((2026, 12, 31), &["pos"])), "pos");
    let (status, _, body) = get(&r, "/pos", &[("x-tenant", "acme")]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("licensed to Acme tenant acme"));

    let r = licensed_app(FixedLicense(license((2026, 11, 1), &[])), "pos");
    let (status, _, body) = get(&r, "/pos", &[]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Your license has expired."));

    let r = licensed_app(FixedLicense(license((2026, 12, 31), &["inventory"])), "pos");
    assert_eq!(get(&r, "/pos", &[]).await.0, StatusCode::FORBIDDEN);
}

struct Flaky(Arc<AtomicUsize>);

impl LicenseSource for Flaky {
    fn load(&self, tenant: Option<&Tenant>) -> BoxFuture<Result<Option<License>, String>> {
        let n = self.0.fetch_add(1, Ordering::SeqCst);
        let holder = tenant.map(|t| t.id().to_string()).unwrap_or_default();
        Box::pin(async move {
            if n == 0 {
                Ok(Some(License { holder, ..license((2026, 12, 31), &[]) }))
            } else {
                Err("license server is down".into())
            }
        })
    }
}

#[tokio::test]
async fn a_failing_license_server_keeps_the_last_license_per_tenant() {
    let calls = Arc::new(AtomicUsize::new(0));
    let r = App::new("POS")
        .fixed_now(DateTime::parse("2026-11-02T10:00:00Z").unwrap())
        .license(Flaky(calls.clone()), Duration::ZERO)
        .tenancy(HeaderTenant("x-tenant"))
        .feature(Feature::new("pos").guard(Licensed::any()).screen(pos))
        .router()
        .unwrap();
    assert!(get(&r, "/pos", &[("x-tenant", "acme")]).await.2.contains("licensed to acme"));
    let (status, _, body) = get(&r, "/pos", &[("x-tenant", "acme")]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("licensed to acme"));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(get(&r, "/pos", &[("x-tenant", "globex")]).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tenant_required_and_identity_tenant_wins() {
    let build = |auth: Option<Identity>| {
        let mut app = App::new("T").tenancy(HeaderTenant("x-tenant")).feature(Feature::new("t").guard(TenantRequired).screen(pos));
        if let Some(id) = auth {
            app = app.auth(move |_: &HeaderMap| Some(id.clone()));
        }
        app.router().unwrap()
    };
    assert_eq!(get(&build(None), "/pos", &[]).await.0, StatusCode::FORBIDDEN);
    assert_eq!(get(&build(None), "/pos", &[("x-tenant", "../x")]).await.0, StatusCode::FORBIDDEN);
    let body = get(&build(Some(Identity::user("a").with_tenant("globex"))), "/pos", &[("x-tenant", "acme")]).await.2;
    assert!(body.contains("tenant globex"));
}

#[tokio::test]
async fn anonymous_users_are_sent_to_the_central_login() {
    let r = App::new("T")
        .login_url("https://login.example.com/authorize?app=t")
        .feature(Feature::new("s").route_prefix("/s").screen(secret_page))
        .router()
        .unwrap();
    let (status, h, _) = get(&r, "/s/secret", &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(h.get(header::LOCATION).unwrap(), "https://login.example.com/authorize?app=t&return_to=%2Fs%2Fsecret");

    let signed_in = App::new("T")
        .login_url("https://login.example.com/authorize")
        .auth(|_: &HeaderMap| Some(Identity::user("x")))
        .feature(Feature::new("s").screen(secret_page))
        .router()
        .unwrap();
    assert_eq!(get(&signed_in, "/secret", &[]).await.0, StatusCode::FORBIDDEN);
}

struct AcmeStandard;

impl Plugin for AcmeStandard {
    fn install(self, app: App) -> App {
        app.auth(ForwardAuth::new("x-user").groups("x-groups").role("admins", ["secret.read"]))
            .time_policy(TimePolicy::Organization(TimeZone::named("Europe/Istanbul").unwrap()))
    }
}

#[tokio::test]
async fn a_plugin_sets_up_a_company_standard_in_one_line() {
    let r = App::new("T").plugin(AcmeStandard).feature(Feature::new("s").screen(secret_page)).router().unwrap();
    let (status, _, body) = get(&r, "/secret", &[("x-user", "ayse"), ("x-groups", "admins")]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<meta name="zeyn-tz" content="Europe/Istanbul">"#));
    assert_eq!(get(&r, "/secret", &[("x-user", "ali"), ("x-groups", "staff")]).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cors_allows_only_listed_origins() {
    let r = App::new("T").cors(["https://app.example.com"]).feature(Feature::new("s").screen(pos)).router().unwrap();
    let pre = |origin: &str| {
        Request::builder()
            .method("OPTIONS")
            .uri("/api/v1/openapi.json")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization")
            .body(Body::empty())
            .unwrap()
    };
    let ok = r.clone().oneshot(pre("https://app.example.com")).await.unwrap();
    assert_eq!(ok.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "https://app.example.com");
    let bad = r.clone().oneshot(pre("https://evil.example.com")).await.unwrap();
    assert!(bad.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    assert!(ok.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
}

fn with_ip(path: &str, ip: &str, method: &str) -> Request<Body> {
    let mut req = Request::builder().method(method).uri(path).body(Body::empty()).unwrap();
    let addr: std::net::SocketAddr = std::net::SocketAddr::new(ip.parse().unwrap(), 5000);
    req.extensions_mut().insert(axum::extract::ConnectInfo(addr));
    req
}

#[tokio::test]
async fn rate_limit_answers_429_with_retry_after_per_client_ip() {
    let r = App::new("T")
        .rate_limit(RateLimit::per_ip(3, Duration::from_secs(60)))
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    for _ in 0..3 {
        assert_eq!(r.clone().oneshot(with_ip("/pos", "203.0.113.5", "GET")).await.unwrap().status(), StatusCode::OK);
    }
    let blocked = r.clone().oneshot(with_ip("/pos", "203.0.113.5", "GET")).await.unwrap();
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers().get(header::RETRY_AFTER).unwrap().to_str().unwrap().parse::<u64>().unwrap() >= 1);
    assert_eq!(blocked.headers().get(header::CONTENT_TYPE).unwrap(), "application/problem+json");
    assert_eq!(r.clone().oneshot(with_ip("/pos", "203.0.113.6", "GET")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(r.clone().oneshot(with_ip("/_zeyn/zeyn.js", "203.0.113.5", "GET")).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn forwarded_for_is_believed_only_from_trusted_proxies() {
    let r = App::new("T")
        .trusted_proxies(["10.0.0.0/8"])
        .rate_limit(RateLimit::per_ip(1, Duration::from_secs(60)))
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    let via = |peer: &str, xff: &str| {
        let mut req = with_ip("/pos", peer, "GET");
        req.headers_mut().insert("x-forwarded-for", xff.parse().unwrap());
        req
    };
    assert_eq!(r.clone().oneshot(via("10.0.0.2", "198.51.100.1, 10.0.0.9")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(r.clone().oneshot(via("10.0.0.3", "198.51.100.2")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(r.clone().oneshot(via("10.0.0.2", "198.51.100.1")).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.clone().oneshot(via("203.0.113.9", "1.1.1.1")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(r.clone().oneshot(via("203.0.113.9", "2.2.2.2")).await.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn ip_lists_block_and_allow_ranges() {
    let r = App::new("T")
        .allow_only_ips(["192.168.0.0/16", "::1"])
        .deny_ips(["192.168.5.5"])
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    assert_eq!(r.clone().oneshot(with_ip("/pos", "192.168.1.20", "GET")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(r.clone().oneshot(with_ip("/pos", "192.168.5.5", "GET")).await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(r.clone().oneshot(with_ip("/pos", "8.8.8.8", "GET")).await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(r.clone().oneshot(with_ip("/pos", "::1", "GET")).await.unwrap().status(), StatusCode::OK);
    let problems = App::new("T").deny_ips(["10.0.0.0/33", "nope"]).check().unwrap_err();
    assert!(problems[0].contains("deny_ips"));
}

#[test]
fn memory_rate_store_caps_its_size() {
    let store = zeyn::MemoryRateStore::with_max_keys(100);
    let limit = RateLimit::per_ip(5, Duration::from_secs(60));
    for i in 0..1000 {
        store.take(&format!("ip:{i}"), &limit).unwrap();
    }
    assert!(store.len() <= 100);
}

#[tokio::test]
async fn api_docs_can_be_closed_open_password_or_permission_protected() {
    let build = |docs: Option<ApiDocs>| {
        let mut app = App::new("T").feature(Feature::new("s").screen(pos));
        if let Some(d) = docs {
            app = app.api_docs(d);
        }
        app.router().unwrap()
    };
    assert_eq!(get(&build(None), "/api/v1/openapi.json", &[]).await.0, StatusCode::NOT_FOUND);
    assert_eq!(get(&build(Some(ApiDocs::open())), "/api/v1/docs", &[]).await.0, StatusCode::OK);

    let basic = build(Some(ApiDocs::basic(StaticCredentials::new("docs", Secret::new("p@ss".into())))));
    let (status, h, _) = get(&basic, "/api/v1/openapi.json", &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(h.get(header::WWW_AUTHENTICATE).unwrap().to_str().unwrap().starts_with("Basic"));
    assert_eq!(get(&basic, "/api/v1/openapi.json", &[("authorization", "Basic ZG9jczp3cm9uZw==")]).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(get(&basic, "/api/v1/openapi.json", &[("authorization", "Basic ZG9jczpwQHNz")]).await.0, StatusCode::OK);
    assert_eq!(get(&basic, "/api/v1/openapi.json", &[("authorization", "Basic !!!")]).await.0, StatusCode::UNAUTHORIZED);

    let perm = App::new("T")
        .auth(|h: &HeaderMap| h.contains_key("x-admin").then(|| Identity::user("a").with_permissions(["api.docs"])))
        .api_docs(ApiDocs::permission("api.docs"))
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    assert_eq!(get(&perm, "/api/v1/docs", &[]).await.0, StatusCode::FORBIDDEN);
    assert_eq!(get(&perm, "/api/v1/docs", &[("x-admin", "1")]).await.0, StatusCode::OK);
}

#[tokio::test]
async fn apps_can_live_under_a_base_path() {
    let build = |app: App| app.home(pos).feature(Feature::new("s").route_prefix("/sales").screen(pos).menu(|m| m.group("S", Icon::Cart).item("POS", pos))).router().unwrap();
    let mounted = build(App::new("T").base_path("inventory/"));
    let (status, h, _) = get(&mounted, "/inventory", &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(h.get(header::LOCATION).unwrap(), "/inventory/sales/pos");
    let (status, h, body) = get(&mounted, "/inventory/sales/pos", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<meta name="zeyn-base" content="/inventory">"#));
    assert!(body.contains(r#"href="/inventory/_zeyn/zeyn.css""#));
    assert!(body.contains(r#"<a href="/inventory/sales/pos" aria-current="page">"#));
    assert!(h.get(header::SET_COOKIE).unwrap().to_str().unwrap().contains("Path=/inventory;"));
    assert_eq!(get(&mounted, "/inventory/_zeyn/zeyn.js", &[]).await.0, StatusCode::OK);
    assert_eq!(get(&mounted, "/sales/pos", &[]).await.0, StatusCode::NOT_FOUND);

    let stripped = build(App::new("T").public_path("/inventory"));
    let (status, _, body) = get(&stripped, "/sales/pos", &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(r#"<a href="/inventory/sales/pos""#));
}

#[tokio::test]
async fn themes_are_served_as_a_stylesheet_and_reject_css_injection() {
    let r = App::new("T")
        .theme(Theme::new().brand("#7a2448").font("Inter, system-ui").radius(40).compact().dark())
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    let (_, _, page) = get(&r, "/pos", &[]).await;
    assert!(page.contains(r#"<html lang="en" data-theme="dark" data-density="compact">"#));
    assert!(page.contains(r#"<link rel="stylesheet" href="/_zeyn/theme.css">"#));
    let (_, h, css) = get(&r, "/_zeyn/theme.css", &[]).await;
    assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "text/css; charset=utf-8");
    assert_eq!(css.trim(), ":root{--z-brand:#7a2448;--z-font:Inter, system-ui;--z-radius:12px;}");

    let evil = App::new("T")
        .theme(Theme::new().brand("red;}body{display:none").font("x;}*{background:url(//evil)}"))
        .feature(Feature::new("s").screen(pos))
        .router()
        .unwrap();
    assert_eq!(get(&evil, "/_zeyn/theme.css", &[]).await.2.trim(), ":root{}");
    assert_eq!(get(&evil, "/favicon.ico", &[]).await.0, StatusCode::NO_CONTENT);
}
