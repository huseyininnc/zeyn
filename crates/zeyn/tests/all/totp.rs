//! Built in TOTP second factor: enrollment with a server drawn QR code, verification with replay
//! protection and a rate limit, hashed recovery codes, and the tie to `RequireMfa` and `reauth`.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{MemoryTotpStore, RequireMfa, Totp, TotpStore};

const KEY: &str = "a totp key for tests only, not used anywhere real";
const CSRF: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[screen(route = "/home", permission = "crm.read")]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[screen(route = "/vault", permission = "crm.read")]
pub fn vault() -> Screen {
    Screen::new("Vault")
}

#[action(permission = "crm.read", reauth)]
pub async fn close_books(year: i64) -> Result<i64> {
    Ok(year)
}

fn app(store: MemoryTotpStore) -> Router {
    app_with(store, 50)
}

fn app_with(store: impl zeyn::TotpStore, attempts: u32) -> Router {
    App::new("T")
        .auth(|h: &HeaderMap| {
            h.get("x-user")
                .and_then(|v| v.to_str().ok())
                .map(|u| Identity::user(u).with_permissions(["crm.read"]))
        })
        .login_url("https://sso.example.com/login")
        .totp(
            Totp::new(store, Secret::new(KEY.into()))
                .issuer("Test App")
                .attempts(attempts, Duration::from_secs(300)),
        )
        .feature(
            Feature::new("crm")
                .screen(home)
                .api(close_books),
        )
        .feature(
            Feature::new("safe")
                .guard(RequireMfa::new())
                .screen(vault),
        )
        .router()
        .unwrap()
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default()
    }

    fn cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|c| c.starts_with(&format!("{name}=")))
    }

    fn cookie_value(&self, name: &str) -> String {
        let c = self
            .cookie(name)
            .unwrap_or_else(|| panic!("no {name} cookie"));
        c[name.len() + 1..]
            .split(';')
            .next()
            .unwrap()
            .to_string()
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

fn get(path: &str, user: &str, cookies: &str) -> Request<Body> {
    Request::get(path)
        .header("x-user", user)
        .header(header::COOKIE, cookies)
        .body(Body::empty())
        .unwrap()
}

fn post(path: &str, user: &str, cookies: &str, csrf: &str, code: &str) -> Request<Body> {
    Request::post(path)
        .header("x-user", user)
        .header(header::COOKIE, format!("zeyn_csrf={CSRF}; {cookies}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "csrf={csrf}&code={code}&return_to=%2Fvault"
        )))
        .unwrap()
}

fn base32_decode(s: &str) -> Vec<u8> {
    let (mut out, mut buffer, mut bits) = (Vec::new(), 0u32, 0u32);
    for c in s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
    {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'2'..=b'7' => c - b'2' + 26,
            _ => panic!("not base32"),
        } as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

fn code(secret: &[u8], offset: i64) -> String {
    let totp = totp_rs::Builder::new()
        .with_secret(secret.to_vec())
        .with_account_name("ayse")
        .with_issuer(Some("Test App"))
        .build()
        .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    totp.generate((now + offset) as u64)
        .to_string()
}

/// Opens the enrollment page and returns the enrollment cookie and the secret shown on it.
async fn enrollment(router: &Router, user: &str) -> (String, Vec<u8>, Reply) {
    let r = send(router, get("/_zeyn/auth/totp?return_to=%2Fvault", user, "")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let start = r
        .body
        .find(r#"<code class="z-totp-key">"#)
        .expect("manual key")
        + 25;
    let key = &r.body[start..start + r.body[start..].find('<').unwrap()];
    (r.cookie_value("zeyn_totp_enroll"), base32_decode(key), r)
}

#[tokio::test]
async fn users_enroll_with_a_qr_code_and_get_recovery_codes_once() {
    let store = MemoryTotpStore::default();
    let router = app(store.clone());
    let r = send(&router, get("/vault", "ayse", "")).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(r.location(), "/_zeyn/auth/totp?return_to=%2Fvault");

    let (pending, secret, page) = enrollment(&router, "ayse").await;
    let (second, other, _) = enrollment(&router, "ayse").await;
    assert_eq!(secret.len(), 20);
    assert!(
        page.body
            .contains(r#"<svg class="z-totp-qr""#)
            && page.body.contains(r#"role="img""#),
        "QR code drawn as SVG"
    );
    assert!(
        page.body
            .contains(r#"autocomplete="one-time-code""#)
    );
    assert!(
        page.cookie("zeyn_totp_enroll")
            .unwrap()
            .contains("HttpOnly")
    );
    assert!(!pending.contains(&format!("{secret:?}")));

    let cookies = format!("zeyn_totp_enroll={pending}");
    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/enroll",
            "ayse",
            &cookies,
            &"f".repeat(64),
            &code(&secret, 0),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::FORBIDDEN,
        "CSRF holds on the enrollment form"
    );
    let r = send(
        &router,
        post("/_zeyn/auth/totp/enroll", "ayse", &cookies, CSRF, "000000"),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/enroll",
            "mehmet",
            &cookies,
            CSRF,
            &code(&secret, 0),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "another user's enrollment cookie does not count"
    );
    assert!(
        store
            .load("mehmet")
            .await
            .unwrap()
            .is_none()
    );

    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/enroll",
            "ayse",
            &cookies,
            CSRF,
            &code(&secret, 0),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let codes: Vec<String> = r
        .body
        .split("<li><code>")
        .skip(1)
        .map(|c| c[..c.find('<').unwrap()].to_string())
        .collect();
    assert_eq!(codes.len(), zeyn::auth::RECOVERY_CODES);
    assert!(
        codes
            .iter()
            .all(|c| c.len() == 11 && c.as_bytes()[5] == b'-')
    );
    let proof = r.cookie_value("zeyn_mfa");
    assert!(
        r.cookie("zeyn_totp_enroll")
            .unwrap()
            .contains("Max-Age=0")
    );

    let record = store
        .load("ayse")
        .await
        .unwrap()
        .unwrap();
    let stored = serde_json::to_string(&record).unwrap();
    assert!(
        codes
            .iter()
            .all(|c| !stored.contains(c.as_str())),
        "recovery codes are stored hashed"
    );
    assert!(
        record
            .recovery
            .iter()
            .all(|h| h.len() == 64)
    );
    assert!(!record.secret.is_empty() && base32_decode_lossy(&record.secret).is_none());

    assert_eq!(
        send(&router, get("/vault", "ayse", &format!("zeyn_mfa={proof}")))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            get("/vault", "mehmet", &format!("zeyn_mfa={proof}"))
        )
        .await
        .status,
        StatusCode::SEE_OTHER,
        "a proof is for one user"
    );

    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/enroll",
            "ayse",
            &format!("zeyn_totp_enroll={second}"),
            CSRF,
            &code(&other, 0),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "an enrolled factor cannot be replaced from a session alone"
    );
    assert_eq!(
        store
            .load("ayse")
            .await
            .unwrap()
            .unwrap(),
        record
    );
}

fn base32_decode_lossy(s: &str) -> Option<Vec<u8>> {
    s.bytes()
        .all(|c| c.is_ascii_uppercase() || (b'2'..=b'7').contains(&c))
        .then(|| base32_decode(s))
}

async fn enrolled(router: &Router, user: &str) -> (Vec<u8>, Vec<String>) {
    let (pending, secret, _) = enrollment(router, user).await;
    let r = send(
        router,
        post(
            "/_zeyn/auth/totp/enroll",
            user,
            &format!("zeyn_totp_enroll={pending}"),
            CSRF,
            &code(&secret, -30),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let codes = r
        .body
        .split("<li><code>")
        .skip(1)
        .map(|c| c[..c.find('<').unwrap()].to_string())
        .collect();
    (secret, codes)
}

/// A store that takes a while to read, as a database does.
#[derive(Clone, Default)]
struct SlowStore(MemoryTotpStore);

impl zeyn::TotpStore for SlowStore {
    fn load(&self, user: &str) -> zeyn::BoxFuture<zeyn::Result<Option<zeyn::TotpRecord>>> {
        let inner = self.0.load(user);
        Box::pin(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            inner.await
        })
    }

    fn save(&self, user: &str, record: zeyn::TotpRecord) -> zeyn::BoxFuture<zeyn::Result<()>> {
        self.0.save(user, record)
    }
}

#[tokio::test]
async fn one_code_passes_once_even_when_sent_twice_at_the_same_time() {
    let router = app_with(SlowStore::default(), 50);
    let (secret, codes) = enrolled(&router, "ayse").await;
    for code in [code(&secret, 0), codes[0].clone()] {
        let (a, b) = tokio::join!(
            send(
                &router,
                post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, &code)
            ),
            send(
                &router,
                post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, &code)
            ),
        );
        let passed = [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::SEE_OTHER)
            .count();
        assert_eq!(passed, 1, "{code}: {} and {}", a.status, b.status);
    }
}

#[tokio::test]
async fn a_refused_csrf_token_is_never_written_into_the_page() {
    let router = app(MemoryTotpStore::default());
    let forged = "%22%3E%3Cmeta+http-equiv%3D%22refresh%22+content%3D%220%3Burl%3Dhttps%3A%2F%2Fevil.example%2F%22%3E";
    let r = send(
        &router,
        post("/_zeyn/auth/totp/verify", "ayse", "", forged, "123456"),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        !r.body.contains("http-equiv") && !r.body.contains("evil.example"),
        "{}",
        r.body
    );
    let meta = r
        .body
        .split(r#"<meta name="zeyn-csrf" content=""#)
        .nth(1)
        .and_then(|t| t.split('"').next())
        .unwrap();
    assert!(
        meta.len() == 128
            && meta
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "the page gets the masked token: {meta}"
    );
}

#[tokio::test]
async fn codes_and_recovery_codes_work_once() {
    let router = app(MemoryTotpStore::default());
    let (secret, codes) = enrolled(&router, "ayse").await;
    let r = send(
        &router,
        get("/_zeyn/auth/totp?return_to=%2Fvault", "ayse", ""),
    )
    .await;
    assert!(
        r.body
            .contains("/_zeyn/auth/totp/verify")
            && !r.body.contains("z-totp-qr"),
        "enrolled users verify"
    );

    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/verify",
            "ayse",
            "",
            CSRF,
            &code(&secret, -30),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "the code used at enrollment is not accepted again"
    );
    let now = code(&secret, 0);
    let r = send(
        &router,
        post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, &now),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    assert_eq!(r.location(), "/vault");
    assert!(
        r.cookie("zeyn_mfa")
            .unwrap()
            .contains("HttpOnly")
    );
    let r = send(
        &router,
        post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, &now),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "a code is accepted once");

    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/verify",
            "ayse",
            "",
            CSRF,
            &codes[3].to_uppercase(),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "recovery code");
    let r = send(
        &router,
        post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, &codes[3]),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "a recovery code works once"
    );
}

#[tokio::test]
async fn verification_is_rate_limited_per_user() {
    let router = app_with(MemoryTotpStore::default(), 5);
    let (secret, _) = enrolled(&router, "ayse").await;
    let mut statuses = Vec::new();
    for _ in 0..5 {
        statuses.push(
            send(
                &router,
                post("/_zeyn/auth/totp/verify", "ayse", "", CSRF, "123456"),
            )
            .await
            .status,
        );
    }
    assert!(
        statuses
            .iter()
            .all(|s| *s == StatusCode::BAD_REQUEST || *s == StatusCode::TOO_MANY_REQUESTS)
    );
    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/verify",
            "ayse",
            "",
            CSRF,
            &code(&secret, 0),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::TOO_MANY_REQUESTS,
        "even a right code waits once the limit is reached"
    );
    let r = send(
        &router,
        post("/_zeyn/auth/totp/verify", "mehmet", "", CSRF, "123456"),
    )
    .await;
    assert_ne!(
        r.status,
        StatusCode::TOO_MANY_REQUESTS,
        "the limit is per user"
    );
}

#[tokio::test]
async fn a_verified_code_counts_as_a_fresh_sign_in_for_reauth_actions() {
    let router = app(MemoryTotpStore::default());
    let call = |cookies: String| {
        Request::post("/api/v1/crm/close_books")
            .header("x-user", "ayse")
            .header(header::COOKIE, cookies)
            .header(header::AUTHORIZATION, "Bearer t")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("2025"))
            .unwrap()
    };
    let r = send(&router, call(String::new())).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.body.contains("reauth_required"), "{}", r.body);
    let (secret, _) = enrolled(&router, "ayse").await;
    let r = send(
        &router,
        post(
            "/_zeyn/auth/totp/verify",
            "ayse",
            "",
            CSRF,
            &code(&secret, 0),
        ),
    )
    .await;
    let proof = r.cookie_value("zeyn_mfa");
    let r = send(&router, call(format!("zeyn_mfa={proof}"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    let page = send(&router, get("/home", "ayse", "")).await;
    assert!(
        page.body
            .contains("&quot;mfa&quot;:&quot;/_zeyn/auth/totp&quot;"),
        "the runtime knows where to step up"
    );
    let r = send(&router, get("/_zeyn/screen/vault", "ayse", "")).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.body.contains("mfa_required"));
}

#[tokio::test]
async fn signed_out_visitors_sign_in_first() {
    let router = app(MemoryTotpStore::default());
    let r = send(
        &router,
        Request::get("/_zeyn/auth/totp")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert!(
        r.location()
            .starts_with("https://sso.example.com/login?return_to=%2F_zeyn%2Fauth%2Ftotp"),
        "{}",
        r.location()
    );
}

#[test]
fn short_keys_stop_the_app() {
    let problems = App::new("T")
        .totp(Totp::new(
            MemoryTotpStore::default(),
            Secret::new("short".into()),
        ))
        .check()
        .unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("Totp")),
        "{problems:?}"
    );
}
