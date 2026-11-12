//! Bearer JWTs, OpenID Connect sign in and step up, against a small fake issuer that signs with a
//! key made for the test run. With `ZEYN_TEST_OIDC` set to a Keycloak address (such as
//! `http://127.0.0.1:8080`, admin `admin`/`admin`), the same flows also run against Keycloak.

use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::server::Authenticator;
use zeyn::{Algorithm, ClaimMap, JwtAuth, OidcAuth, RequireMfa};

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn query(url: &str) -> HashMap<String, String> {
    let q = url
        .split_once('?')
        .map(|(_, q)| q)
        .unwrap_or("");
    q.split('&')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_string(), percent_decode(v)))
        .collect()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(if b[i] == b'+' { b' ' } else { b[i] });
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

/// A fake OpenID Connect issuer on a local port.
struct Idp {
    base: String,
    key: EcdsaKeyPair,
    kid: String,
    codes: Mutex<HashMap<String, (String, String, Value)>>,
    jwks_hits: AtomicUsize,
    refresh_ok: AtomicBool,
    expires_in: AtomicI64,
}

impl Idp {
    async fn start() -> Arc<Idp> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let idp = Arc::new(Idp {
            base,
            key,
            kid: "test-key-1".into(),
            codes: Mutex::default(),
            jwks_hits: AtomicUsize::new(0),
            refresh_ok: AtomicBool::new(true),
            expires_in: AtomicI64::new(300),
        });
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(idp.clone());
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .unwrap()
        });
        idp
    }

    fn jwk(&self) -> Value {
        let p = self.key.public_key().as_ref();
        json!({ "kty": "EC", "crv": "P-256", "kid": self.kid, "use": "sig", "alg": "ES256", "x": b64(&p[1..33]), "y": b64(&p[33..65]) })
    }

    fn claims(&self, aud: &str, extra: Value) -> Value {
        let mut c = json!({
            "iss": self.base, "aud": aud, "sub": "u-1", "preferred_username": "ayse",
            "exp": now() + 300, "iat": now(), "realm_access": { "roles": ["clerks", "unknown"] }, "amr": ["pwd"],
        });
        for (k, v) in extra.as_object().unwrap() {
            c[k] = v.clone();
        }
        c
    }

    fn sign(&self, header: Value, claims: &Value) -> String {
        let input = format!(
            "{}.{}",
            b64(header.to_string().as_bytes()),
            b64(claims.to_string().as_bytes())
        );
        let sig = self
            .key
            .sign(&SystemRandom::new(), input.as_bytes())
            .unwrap();
        format!("{input}.{}", b64(sig.as_ref()))
    }

    fn token(&self, aud: &str, extra: Value) -> String {
        self.sign(
            json!({ "alg": "ES256", "typ": "JWT", "kid": self.kid }),
            &self.claims(aud, extra),
        )
    }

    fn register(&self, code: &str, nonce: &str, challenge: &str, extra: Value) {
        self.codes
            .lock()
            .unwrap()
            .insert(code.into(), (nonce.into(), challenge.into(), extra));
    }
}

async fn discovery(State(idp): State<Arc<Idp>>) -> Json<Value> {
    let b = &idp.base;
    Json(json!({
        "issuer": b, "authorization_endpoint": format!("{b}/authorize"), "token_endpoint": format!("{b}/token"),
        "jwks_uri": format!("{b}/jwks"), "end_session_endpoint": format!("{b}/logout"),
    }))
}

async fn jwks(State(idp): State<Arc<Idp>>) -> Json<Value> {
    idp.jwks_hits
        .fetch_add(1, Ordering::SeqCst);
    Json(json!({ "keys": [idp.jwk(), { "kty": "oct", "k": "AAAA", "kid": "shared" }] }))
}

async fn token(
    State(idp): State<Arc<Idp>>,
    headers: HeaderMap,
    Form(f): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let basic = format!("Basic {}", base64_std(b"web:not-a-real-secret"));
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        != Some(basic.as_str())
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid_client" })),
        );
    }
    let bad = (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "invalid_grant" })),
    );
    let expires_in = idp.expires_in.load(Ordering::SeqCst);
    match f.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            let Some((nonce, challenge, extra)) = idp.codes.lock().unwrap().remove(
                f.get("code")
                    .map(String::as_str)
                    .unwrap_or(""),
            ) else {
                return bad;
            };
            let verifier = f
                .get("code_verifier")
                .cloned()
                .unwrap_or_default();
            if b64(ring::digest::digest(&ring::digest::SHA256, verifier.as_bytes()).as_ref())
                != challenge
            {
                return bad;
            }
            if f.get("redirect_uri")
                .map(String::as_str)
                != Some("http://app.test/_zeyn/auth/callback")
            {
                return bad;
            }
            let mut extra = extra;
            if extra.get("nonce").is_none() {
                extra["nonce"] = json!(nonce);
            }
            (
                StatusCode::OK,
                Json(
                    json!({ "id_token": idp.token("web", extra), "access_token": "opaque", "expires_in": expires_in, "refresh_token": "r1" }),
                ),
            )
        }
        Some("refresh_token")
            if idp.refresh_ok.load(Ordering::SeqCst)
                && f.get("refresh_token")
                    .is_some_and(|r| r.starts_with('r')) =>
        {
            (
                StatusCode::OK,
                Json(
                    json!({ "id_token": idp.token("web", json!({})), "expires_in": expires_in, "refresh_token": "r2" }),
                ),
            )
        }
        _ => bad,
    }
}

fn base64_std(bytes: &[u8]) -> String {
    b64(bytes)
        .replace('-', "+")
        .replace('_', "/")
        + &"=".repeat((3 - bytes.len() % 3) % 3)
}

#[screen(route = "/home", permission = "crm.read")]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[screen(route = "/vault", permission = "crm.read")]
pub fn vault() -> Screen {
    Screen::new("Vault").body(Alert::info("Vault contents"))
}

#[screen(route = "/payroll", permission = "crm.read")]
pub fn payroll() -> Screen {
    Screen::new("Payroll")
}

const KEY: &str = "a session key for tests only, not used anywhere real";

fn oidc(idp: &Idp, public: &str) -> OidcAuth {
    OidcAuth::new(&idp.base, "web", public, Secret::new(KEY.into()))
        .client_secret(Secret::new("not-a-real-secret".into()))
        .algorithms([Algorithm::ES256])
        .claims(
            ClaimMap::new()
                .roles("realm_access.roles")
                .role("clerks", ["crm.read"])
                .mfa_acr(["2"]),
        )
}

fn app(auth: OidcAuth) -> Router {
    App::new("T")
        .oidc(auth)
        .interceptor(RequireMfa::new().screen("vault"))
        .feature(
            Feature::new("crm")
                .screen(home)
                .screen(vault),
        )
        .feature(
            Feature::new("pay")
                .guard(RequireMfa::new())
                .screen(payroll),
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

fn get_with(path: &str, cookies: &str) -> Request<Body> {
    Request::get(path)
        .header(header::COOKIE, cookies)
        .body(Body::empty())
        .unwrap()
}

/// Starts a sign in and returns the flow cookie and the provider's query parameters.
async fn start(router: &Router, path: &str) -> (String, HashMap<String, String>, Reply) {
    let r = send(router, get_with(path, "")).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    let flow = r.cookie_value("zeyn_oidc");
    let q = query(&r.location());
    (flow, q, r)
}

/// A full sign in; the ID token gets `extra` claims. Returns the session cookie value.
async fn sign_in(router: &Router, idp: &Idp, path: &str, code: &str, extra: Value) -> String {
    let (flow, q, _) = start(router, path).await;
    idp.register(code, &q["nonce"], &q["code_challenge"], extra);
    let r = send(
        router,
        get_with(
            &format!("/_zeyn/auth/callback?code={code}&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    r.cookie_value("zeyn_session")
}

#[tokio::test]
async fn a_browser_signs_in_with_pkce_state_and_nonce_and_gets_a_sealed_session_cookie() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let r = send(&router, get_with("/home", "")).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(r.location(), "/_zeyn/auth/login?return_to=%2Fhome");

    let (flow, q, r) = start(&router, "/_zeyn/auth/login?return_to=%2Fhome").await;
    assert!(
        r.location()
            .starts_with(&format!("{}/authorize?", idp.base))
    );
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["client_id"], "web");
    assert_eq!(q["redirect_uri"], "http://app.test/_zeyn/auth/callback");
    assert_eq!(q["code_challenge_method"], "S256");
    assert!(
        q["scope"]
            .split(' ')
            .any(|s| s == "openid")
    );
    assert!(q["state"].len() >= 43 && q["nonce"].len() >= 43 && q["state"] != q["nonce"]);
    assert!(!q.contains_key("prompt"));
    let flow_cookie = r.cookie("zeyn_oidc").unwrap();
    assert!(
        flow_cookie.contains("HttpOnly")
            && flow_cookie.contains("SameSite=Lax")
            && !flow_cookie.contains("Secure"),
        "{flow_cookie}"
    );
    assert!(!flow.contains(&q["state"]), "the flow cookie is encrypted");

    idp.register("c1", &q["nonce"], &q["code_challenge"], json!({}));
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c1&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    assert_eq!(r.location(), "/home");
    let session = r.cookie("zeyn_session").unwrap();
    assert!(
        session.contains("HttpOnly")
            && session.contains("SameSite=Lax")
            && session.contains("Path=/"),
        "{session}"
    );
    assert!(
        r.cookie("zeyn_oidc")
            .unwrap()
            .contains("Max-Age=0"),
        "the flow cookie is used once"
    );
    let value = r.cookie_value("zeyn_session");
    assert!(!value.contains("ayse") && value.len() < 3800);

    let r = send(&router, get_with("/home", &format!("zeyn_session={value}"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(
        r.body.contains("_zeyn/auth/logout"),
        "idle sign out goes through the provider"
    );

    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c1&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "a replayed callback is refused"
    );
    assert!(r.cookie("zeyn_session").is_none());
}

#[tokio::test]
async fn https_apps_get_secure_cookies() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "https://app.test"));
    let (_, q, r) = start(&router, "/_zeyn/auth/login").await;
    assert!(
        r.cookie("zeyn_oidc")
            .unwrap()
            .contains("; Secure")
    );
    assert_eq!(q["redirect_uri"], "https://app.test/_zeyn/auth/callback");
}

#[tokio::test]
async fn callbacks_with_a_wrong_state_nonce_or_cookie_are_refused() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));

    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    idp.register("c2", &q["nonce"], &q["code_challenge"], json!({}));
    let r = send(
        &router,
        get_with(
            "/_zeyn/auth/callback?code=c2&state=forged",
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "mismatched state");
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c2&state={}", q["state"]),
            "",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "no flow cookie");
    let mut tampered = flow.clone().into_bytes();
    let i = tampered.len() / 2;
    tampered[i] = if tampered[i] == b'A' { b'B' } else { b'A' };
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c2&state={}", q["state"]),
            &format!("zeyn_oidc={}", String::from_utf8(tampered).unwrap()),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "tampered flow cookie");

    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    idp.register(
        "c3",
        "another nonce",
        &q["code_challenge"],
        json!({ "nonce": "another nonce" }),
    );
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c3&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "nonce mismatch");
    assert!(r.cookie("zeyn_session").is_none());

    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    idp.register(
        "c4",
        &q["nonce"],
        "a challenge for another verifier",
        json!({}),
    );
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c4&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "PKCE verifier mismatch");

    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    let r = send(
        &router,
        get_with(
            &format!(
                "/_zeyn/auth/callback?error=access_denied&state={}",
                q["state"]
            ),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "provider error");

    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    idp.register(
        "c5",
        &q["nonce"],
        &q["code_challenge"],
        json!({ "aud": ["web", "other"], "azp": "other" }),
    );
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=c5&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "ID token for another authorized party"
    );
}

#[tokio::test]
async fn return_to_only_accepts_local_paths() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    for target in [
        "https%3A%2F%2Fevil.example%2F",
        "%2F%2Fevil.example",
        "%2F%5Cevil.example",
    ] {
        let (flow, q, _) = start(&router, &format!("/_zeyn/auth/login?return_to={target}")).await;
        let code = format!("r-{target}");
        idp.register(&code, &q["nonce"], &q["code_challenge"], json!({}));
        let r = send(
            &router,
            get_with(
                &format!(
                    "/_zeyn/auth/callback?code={}&state={}",
                    percent_encode(&code),
                    q["state"]
                ),
                &format!("zeyn_oidc={flow}"),
            ),
        )
        .await;
        assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
        assert_eq!(r.location(), "/", "{target}");
    }
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'-' {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[tokio::test]
async fn a_tampered_or_foreign_session_cookie_is_not_signed_in() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let value = sign_in(&router, &idp, "/_zeyn/auth/login", "c5", json!({})).await;
    let mut bytes = value.clone().into_bytes();
    let i = bytes.len() - 5;
    bytes[i] = if bytes[i] == b'x' { b'y' } else { b'x' };
    let r = send(
        &router,
        get_with(
            "/home",
            &format!("zeyn_session={}", String::from_utf8(bytes).unwrap()),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "tampered session goes to sign in"
    );

    let other = App::new("T")
        .oidc(
            OidcAuth::new(
                &idp.base,
                "web",
                "http://app.test",
                Secret::new("another key of at least thirty two bytes".into()),
            )
            .algorithms([Algorithm::ES256]),
        )
        .feature(Feature::new("crm").screen(home))
        .router()
        .unwrap();
    let r = send(&other, get_with("/home", &format!("zeyn_session={value}"))).await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "a cookie sealed with another key does not open"
    );
}

#[tokio::test]
async fn step_up_sends_the_user_back_to_the_provider_with_acr_values() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let plain = sign_in(&router, &idp, "/_zeyn/auth/login", "s1", json!({})).await;
    let cookie = format!("zeyn_session={plain}");
    assert_eq!(
        send(&router, get_with("/home", &cookie))
            .await
            .status,
        StatusCode::OK
    );

    for path in ["/vault", "/payroll"] {
        let r = send(&router, get_with(path, &cookie)).await;
        assert_eq!(r.status, StatusCode::SEE_OTHER, "{path}: {}", r.body);
        assert_eq!(
            r.location(),
            format!("/_zeyn/auth/login?mfa=1&return_to=%2F{}", &path[1..])
        );
    }
    let r = send(&router, get_with("/_zeyn/screen/vault", &cookie)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(r.body.contains("mfa_required"), "{}", r.body);
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let node = Request::post("/_zeyn/node")
        .header(header::COOKIE, format!("{cookie}; zeyn_csrf={token}"))
        .header("x-zeyn-csrf", token)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "screen": "vault", "node": "0" }).to_string(),
        ))
        .unwrap();
    let r = send(&router, node).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "lazy parts of a screen run the interceptors too"
    );
    assert!(!r.body.contains("Vault contents"), "{}", r.body);

    let (flow, q, _) = start(&router, "/_zeyn/auth/login?mfa=1&return_to=%2Fvault").await;
    assert_eq!(q["acr_values"], "2");
    assert_eq!(q["prompt"], "login");
    assert_eq!(q["max_age"], "0");
    idp.register("s2", &q["nonce"], &q["code_challenge"], json!({}));
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=s2&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::BAD_REQUEST,
        "the provider did not do MFA"
    );

    let strong = sign_in(
        &router,
        &idp,
        "/_zeyn/auth/login?mfa=1&return_to=%2Fvault",
        "s3",
        json!({ "amr": ["pwd", "otp"] }),
    )
    .await;
    let r = send(
        &router,
        get_with("/vault", &format!("zeyn_session={strong}")),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let by_acr = sign_in(
        &router,
        &idp,
        "/_zeyn/auth/login?mfa=1",
        "s4",
        json!({ "acr": "2" }),
    )
    .await;
    assert_eq!(
        send(
            &router,
            get_with("/payroll", &format!("zeyn_session={by_acr}"))
        )
        .await
        .status,
        StatusCode::OK
    );

    let r = send(&router, get_with("/_zeyn/auth/login?reauth=1", "")).await;
    let q = query(&r.location());
    assert_eq!(
        (q["prompt"].as_str(), q["max_age"].as_str()),
        ("login", "0")
    );
    assert!(!q.contains_key("acr_values"));
}

#[tokio::test]
async fn csrf_still_guards_posts_for_cookie_sessions() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let value = sign_in(&router, &idp, "/_zeyn/auth/login", "x1", json!({})).await;
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let post = |csrf: Option<&str>| {
        let mut b = Request::post("/_zeyn/keepalive").header(
            header::COOKIE,
            format!("zeyn_session={value}; zeyn_csrf={token}"),
        );
        if let Some(c) = csrf {
            b = b.header("x-zeyn-csrf", c);
        }
        b.body(Body::empty()).unwrap()
    };
    assert_eq!(
        send(&router, post(None)).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, post(Some("f".repeat(64).as_str())))
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, post(Some(token)))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn logout_clears_the_session_and_goes_through_the_end_session_endpoint() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let value = sign_in(&router, &idp, "/_zeyn/auth/login", "l1", json!({})).await;
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let r = send(
        &router,
        Request::post("/_zeyn/auth/logout")
            .header(
                header::COOKIE,
                format!("zeyn_session={value}; zeyn_csrf={token}"),
            )
            .header("x-zeyn-csrf", token)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert!(
        r.cookie("zeyn_session")
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        r.headers
            .get("clear-site-data")
            .map(|v| v.to_str().unwrap()),
        Some(r#""storage""#),
        "signing out clears the browser storage of the app"
    );
    let r = send(&router, get_with("/_zeyn/auth/logout", "")).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(
        r.location(),
        format!(
            "{}/logout?client_id=web&post_logout_redirect_uri=http%3A%2F%2Fapp.test%2F",
            idp.base
        )
    );
}

#[tokio::test]
async fn a_cross_site_get_or_a_post_without_csrf_does_not_sign_out() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test"));
    let value = sign_in(&router, &idp, "/_zeyn/auth/login", "l2", json!({})).await;
    let r = send(
        &router,
        get_with("/_zeyn/auth/logout", &format!("zeyn_session={value}")),
    )
    .await;
    assert_eq!(r.status, StatusCode::SEE_OTHER);
    assert_eq!(
        r.location(),
        "http://app.test/",
        "a signed in GET goes back to the app"
    );
    assert!(r.cookie("zeyn_session").is_none(), "GET keeps the session");
    assert!(
        r.headers
            .get("clear-site-data")
            .is_none()
    );
    let r = send(
        &router,
        Request::post("/_zeyn/auth/logout")
            .header(header::COOKIE, format!("zeyn_session={value}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(
        r.cookie("zeyn_session").is_none(),
        "a refused POST keeps the session"
    );
}

#[tokio::test]
async fn refresh_tokens_renew_the_session_and_a_refused_refresh_signs_out() {
    let idp = Idp::start().await;
    idp.expires_in
        .store(0, Ordering::SeqCst);
    let router = app(oidc(&idp, "http://app.test").refresh_tokens(true));
    let value = sign_in(&router, &idp, "/_zeyn/auth/login", "f1", json!({})).await;
    let r = send(&router, get_with("/home", &format!("zeyn_session={value}"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let renewed = r.cookie_value("zeyn_session");
    assert_ne!(renewed, value);

    idp.refresh_ok
        .store(false, Ordering::SeqCst);
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let keepalive = Request::post("/_zeyn/keepalive")
        .header(
            header::COOKIE,
            format!("zeyn_session={renewed}; zeyn_csrf={token}"),
        )
        .header("x-zeyn-csrf", token)
        .body(Body::empty())
        .unwrap();
    let r = send(&router, keepalive).await;
    assert!(
        r.cookie("zeyn_session")
            .is_some_and(|c| c.contains("Max-Age=0")),
        "every route refreshes, also those added after sign in: {:?}",
        r.headers
    );
    let r = send(
        &router,
        get_with("/home", &format!("zeyn_session={renewed}")),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::SEE_OTHER,
        "refused refresh: signed out"
    );
    assert!(
        r.cookie("zeyn_session")
            .unwrap()
            .contains("Max-Age=0")
    );
}

#[tokio::test]
async fn an_oversized_session_is_refused_instead_of_cut() {
    let idp = Idp::start().await;
    let auth = oidc(&idp, "http://app.test").claims(ClaimMap::new().permissions("scope"));
    let router = app(auth);
    let (flow, q, _) = start(&router, "/_zeyn/auth/login").await;
    let scope: Vec<String> = (0..400)
        .map(|i| format!("crm.permission.number.{i}"))
        .collect();
    idp.register(
        "big",
        &q["nonce"],
        &q["code_challenge"],
        json!({ "scope": scope.join(" ") }),
    );
    let r = send(
        &router,
        get_with(
            &format!("/_zeyn/auth/callback?code=big&state={}", q["state"]),
            &format!("zeyn_oidc={flow}"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert!(r.cookie("zeyn_session").is_none());
}

#[test]
fn short_session_keys_stop_the_app() {
    let auth = OidcAuth::new(
        "https://sso.example.com",
        "web",
        "https://app.example.com",
        Secret::new("short".into()),
    );
    let problems = App::new("T")
        .oidc(auth)
        .feature(Feature::new("crm").screen(home))
        .check()
        .unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("session key")),
        "{problems:?}"
    );
}

fn bearer(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}")
            .parse()
            .unwrap(),
    );
    h
}

async fn check(auth: &JwtAuth, token: &str) -> Option<Identity> {
    let h = bearer(token);
    if let Some(ready) = auth.prepare(&h) {
        ready.await;
    }
    auth.authenticate(&h)
}

fn jwt(idp: &Idp) -> JwtAuth {
    JwtAuth::new(&idp.base, "api")
        .algorithms([Algorithm::ES256])
        .claims(
            ClaimMap::new()
                .roles("realm_access.roles")
                .role("clerks", ["crm.read"])
                .tenant("tenant"),
        )
}

#[tokio::test]
async fn bearer_tokens_map_claims_to_the_identity() {
    let idp = Idp::start().await;
    let auth = jwt(&idp);
    let id = check(
        &auth,
        &idp.token(
            "api",
            json!({ "tenant": "acme", "zoneinfo": "Europe/Istanbul", "auth_time": 1_700_000_000 }),
        ),
    )
    .await
    .unwrap();
    assert_eq!(id.user.as_deref(), Some("ayse"));
    assert_eq!(id.tenant.as_deref(), Some("acme"));
    assert_eq!(id.time_zone.as_deref(), Some("Europe/Istanbul"));
    assert!(id.permissions.contains("crm.read") && id.permissions.len() == 1);
    assert_eq!(
        id.authenticated_at,
        Some(DateTime::from_unix_ms(1_700_000_000_000))
    );
    assert!(!id.mfa);
    assert!(
        check(&auth, &idp.token("api", json!({ "amr": ["pwd", "hwk"] })))
            .await
            .unwrap()
            .mfa
    );
    let by_list = check(
        &auth,
        &idp.token("api", json!({ "aud": ["other", "api"], "azp": "api" })),
    )
    .await;
    assert!(by_list.is_some());
    assert!(
        check(&auth, &idp.token("api", json!({ "exp": now() - 30 })))
            .await
            .is_some(),
        "inside the leeway"
    );
    let many = check(
        &auth,
        &idp.token("api", json!({ "tenant": ["acme", "globex"] })),
    )
    .await
    .unwrap();
    assert_eq!(many.tenant.as_deref(), Some("acme"));
    assert!(many.member_of("globex") && !many.member_of("initech"));
}

#[tokio::test]
async fn bearer_tokens_that_are_expired_or_meant_for_others_are_refused() {
    let idp = Idp::start().await;
    let auth = jwt(&idp);
    assert!(
        check(&auth, &idp.token("api", json!({})))
            .await
            .is_some()
    );
    let refused = [
        ("expired", idp.token("api", json!({ "exp": now() - 600 }))),
        ("no exp", idp.token("api", json!({ "exp": null }))),
        (
            "not yet valid",
            idp.token("api", json!({ "nbf": now() + 600 })),
        ),
        (
            "issued in the future",
            idp.token("api", json!({ "iat": now() + 600 })),
        ),
        ("wrong audience", idp.token("other", json!({}))),
        (
            "wrong issuer",
            idp.token("api", json!({ "iss": "https://evil.example" })),
        ),
        (
            "no user",
            idp.token("api", json!({ "sub": null, "preferred_username": null })),
        ),
    ];
    for (why, token) in refused {
        assert!(check(&auth, &token).await.is_none(), "{why}");
    }
}

#[tokio::test]
async fn forged_tokens_are_refused() {
    let idp = Idp::start().await;
    let auth = jwt(&idp);
    let claims = idp.claims("api", json!({}));
    let body = b64(claims.to_string().as_bytes());

    let none = format!("{}.{body}.", b64(br#"{"alg":"none","typ":"JWT"}"#));
    assert!(check(&auth, &none).await.is_none(), "alg none");

    let public = idp.key.public_key().as_ref().to_vec();
    let jwk = idp.jwk().to_string();
    for secret in [public.as_slice(), jwk.as_bytes(), b"AAAA".as_slice()] {
        let header =
            b64(format!(r#"{{"alg":"HS256","typ":"JWT","kid":"{}"}}"#, idp.kid).as_bytes());
        let input = format!("{header}.{body}");
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret);
        let forged = format!(
            "{input}.{}",
            b64(ring::hmac::sign(&key, input.as_bytes()).as_ref())
        );
        assert!(
            check(&auth, &forged).await.is_none(),
            "HS256 with the public key as secret"
        );
    }

    let good = idp.token("api", json!({}));
    let (head, rest) = good.split_once('.').unwrap();
    let sig = rest.split_once('.').unwrap().1;
    let more = idp.claims(
        "api",
        json!({ "realm_access": { "roles": ["clerks", "admins"] }, "preferred_username": "root" }),
    );
    let tampered = format!("{head}.{}.{sig}", b64(more.to_string().as_bytes()));
    assert!(check(&auth, &tampered).await.is_none(), "changed claims");

    let crit = idp.sign(
        json!({ "alg": "ES256", "kid": idp.kid, "crit": ["exp"] }),
        &claims,
    );
    assert!(
        check(&auth, &crit).await.is_none(),
        "unknown critical header"
    );
    let rs = jwt(&idp).algorithms([Algorithm::RS256]);
    assert!(check(&rs, &good).await.is_none(), "algorithm not allowed");
    assert!(
        check(&auth, "not.a.token")
            .await
            .is_none()
    );
    assert!(
        check(&auth, &"a".repeat(20_000))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn keys_are_cached_and_unknown_key_ids_refresh_at_a_limited_rate() {
    let idp = Idp::start().await;
    let auth = jwt(&idp).min_key_refresh(Duration::from_secs(60));
    for _ in 0..3 {
        assert!(
            check(&auth, &idp.token("api", json!({})))
                .await
                .is_some()
        );
    }
    assert_eq!(idp.jwks_hits.load(Ordering::SeqCst), 1, "keys are cached");
    let claims = idp.claims("api", json!({}));
    for i in 0..5 {
        let t = idp.sign(
            json!({ "alg": "ES256", "kid": format!("made-up-{i}") }),
            &claims,
        );
        assert!(check(&auth, &t).await.is_none());
    }
    assert_eq!(
        idp.jwks_hits.load(Ordering::SeqCst),
        1,
        "made up key ids cannot flood the issuer"
    );

    let quick = jwt(&idp).min_key_refresh(Duration::ZERO);
    assert!(
        check(&quick, &idp.token("api", json!({})))
            .await
            .is_some()
    );
    let before = idp.jwks_hits.load(Ordering::SeqCst);
    let t = idp.sign(json!({ "alg": "ES256", "kid": "rotated" }), &claims);
    assert!(check(&quick, &t).await.is_none());
    assert_eq!(
        idp.jwks_hits.load(Ordering::SeqCst),
        before + 1,
        "an unknown key id fetches the keys again"
    );
}

#[tokio::test]
async fn api_and_mcp_calls_accept_bearer_tokens_next_to_the_browser_session() {
    let idp = Idp::start().await;
    let router = app(oidc(&idp, "http://app.test").accept_bearer("api"));
    let page = |auth: String| {
        Request::get("/home")
            .header(header::AUTHORIZATION, auth)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(
            &router,
            page(format!("Bearer {}", idp.token("api", json!({}))))
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            page(format!("Bearer {}", idp.token("web", json!({}))))
        )
        .await
        .status,
        StatusCode::SEE_OTHER
    );
}

mod keycloak {
    //! The same flows against a real Keycloak in development mode:
    //! `docker run -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev`
    //! then `ZEYN_TEST_OIDC=http://127.0.0.1:8080`.

    use super::*;

    const REALM: &str = "zeyn-test";
    const CLIENT_SECRET: &str = "not-a-real-secret";

    async fn admin_token(http: &reqwest::Client, base: &str) -> String {
        let r: Value = http
            .post(format!(
                "{base}/realms/master/protocol/openid-connect/token"
            ))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&[
                ("grant_type", "password"),
                ("client_id", "admin-cli"),
                ("username", "admin"),
                ("password", "admin"),
            ]))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        r["access_token"]
            .as_str()
            .expect("admin token")
            .to_string()
    }

    /// Creates the test realm with a confidential client, an audience mapper and a user.
    async fn setup(http: &reqwest::Client, base: &str) {
        let token = admin_token(http, base).await;
        let _ = http
            .delete(format!("{base}/admin/realms/{REALM}"))
            .bearer_auth(&token)
            .send()
            .await;
        let realm = json!({
            "realm": REALM, "enabled": true,
            "roles": { "realm": [{ "name": "clerks" }] },
            "clients": [{
                "clientId": "zeyn-web", "enabled": true, "publicClient": false, "secret": CLIENT_SECRET,
                "redirectUris": ["http://app.test/_zeyn/auth/callback"], "webOrigins": ["+"],
                "standardFlowEnabled": true, "directAccessGrantsEnabled": true,
                "attributes": { "pkce.code.challenge.method": "S256", "post.logout.redirect.uris": "http://app.test/" },
                "protocolMappers": [{
                    "name": "realm roles in the ID token", "protocol": "openid-connect", "protocolMapper": "oidc-usermodel-realm-role-mapper",
                    "config": { "claim.name": "realm_access.roles", "multivalued": "true", "id.token.claim": "true", "access.token.claim": "true", "jsonType.label": "String" }
                }, {
                    "name": "api audience", "protocol": "openid-connect", "protocolMapper": "oidc-audience-mapper",
                    "config": { "included.custom.audience": "zeyn-api", "access.token.claim": "true", "id.token.claim": "false" }
                }]
            }],
            "users": [{
                "username": "ayse", "enabled": true, "email": "ayse@example.com", "emailVerified": true,
                "firstName": "Ayse", "lastName": "Test",
                "credentials": [{ "type": "password", "value": "test-password", "temporary": false }],
                "realmRoles": ["clerks", "default-roles-zeyn-test"]
            }]
        });
        let r = http
            .post(format!("{base}/admin/realms"))
            .bearer_auth(&token)
            .json(&realm)
            .send()
            .await
            .unwrap();
        assert!(
            r.status().is_success(),
            "realm import: {}",
            r.text().await.unwrap_or_default()
        );
    }

    fn form_body(pairs: &[(&str, &str)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    fn cookies_of(res: &reqwest::Response, jar: &mut HashMap<String, String>) {
        for c in res
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
        {
            let c = c.to_str().unwrap();
            if let Some((k, v)) = c
                .split(';')
                .next()
                .unwrap()
                .split_once('=')
            {
                jar.insert(k.trim().into(), v.into());
            }
        }
    }

    fn jar_header(jar: &HashMap<String, String>) -> String {
        jar.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn html_decode(s: &str) -> String {
        s.replace("&amp;", "&")
            .replace("&#61;", "=")
            .replace("&quot;", "\"")
    }

    /// Plays the browser at Keycloak: opens the authorization URL, posts the login form, and
    /// returns where Keycloak redirects (the app's callback).
    async fn browser_login(http: &reqwest::Client, authorize: &str) -> String {
        let mut jar = HashMap::new();
        let res = http
            .get(authorize)
            .send()
            .await
            .unwrap();
        cookies_of(&res, &mut jar);
        let html = res.text().await.unwrap();
        let start = html
            .find("action=\"")
            .expect("login form")
            + 8;
        let action = html_decode(&html[start..start + html[start..].find('"').unwrap()]);
        let res = http
            .post(&action)
            .header(reqwest::header::COOKIE, jar_header(&jar))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&[
                ("username", "ayse"),
                ("password", "test-password"),
                ("credentialId", ""),
            ]))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), reqwest::StatusCode::FOUND, "login form post");
        res.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn sign_in_step_up_and_bearer_tokens_work_with_keycloak() {
        let Ok(base) = std::env::var("ZEYN_TEST_OIDC") else {
            eprintln!("skipped: set ZEYN_TEST_OIDC to a Keycloak address to run");
            return;
        };
        let base = base.trim_end_matches('/').to_string();
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        setup(&http, &base).await;
        let issuer = format!("{base}/realms/{REALM}");
        let claims = ClaimMap::new()
            .roles("realm_access.roles")
            .role("clerks", ["crm.read"])
            .mfa_acr(["2"]);
        let router = app(OidcAuth::new(
            &issuer,
            "zeyn-web",
            "http://app.test",
            Secret::new(KEY.into()),
        )
        .client_secret(Secret::new(CLIENT_SECRET.into()))
        .claims(claims.clone())
        .refresh_tokens(true));

        let (flow, q, r) = start(&router, "/_zeyn/auth/login?return_to=%2Fhome").await;
        let back = browser_login(&http, &r.location()).await;
        assert!(
            back.starts_with("http://app.test/_zeyn/auth/callback?"),
            "{back}"
        );
        let callback = back.trim_start_matches("http://app.test");
        assert_eq!(query(callback)["state"], q["state"]);
        let r = send(&router, get_with(callback, &format!("zeyn_oidc={flow}"))).await;
        assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
        assert_eq!(r.location(), "/home");
        let session = r.cookie_value("zeyn_session");
        let r = send(
            &router,
            get_with("/home", &format!("zeyn_session={session}")),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);

        let r = send(&router, get_with(callback, &format!("zeyn_oidc={flow}"))).await;
        assert_eq!(
            r.status,
            StatusCode::BAD_REQUEST,
            "Keycloak refuses a reused code"
        );

        let r = send(
            &router,
            get_with("/vault", &format!("zeyn_session={session}")),
        )
        .await;
        assert_eq!(r.location(), "/_zeyn/auth/login?mfa=1&return_to=%2Fvault");
        let (flow, q, r) = start(&router, "/_zeyn/auth/login?mfa=1&return_to=%2Fvault").await;
        assert_eq!(q["acr_values"], "2");
        let back = browser_login(&http, &r.location()).await;
        let r = send(
            &router,
            get_with(
                back.trim_start_matches("http://app.test"),
                &format!("zeyn_oidc={flow}"),
            ),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::BAD_REQUEST,
            "no second factor was done, so step up is refused"
        );

        let r = send(
            &router,
            get_with("/_zeyn/auth/logout", &format!("zeyn_session={session}")),
        )
        .await;
        assert!(
            r.location().starts_with(&format!(
                "{issuer}/protocol/openid-connect/logout?client_id=zeyn-web"
            )),
            "{}",
            r.location()
        );

        let tokens: Value = http
            .post(format!("{issuer}/protocol/openid-connect/token"))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body(&[
                ("grant_type", "password"),
                ("client_id", "zeyn-web"),
                ("client_secret", CLIENT_SECRET),
                ("username", "ayse"),
                ("password", "test-password"),
                ("scope", "openid"),
            ]))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let access = tokens["access_token"]
            .as_str()
            .expect("access token");
        let api = JwtAuth::new(&issuer, "zeyn-api").claims(claims.clone());
        let id = check(&api, access)
            .await
            .expect("Keycloak access token accepted");
        assert_eq!(id.user.as_deref(), Some("ayse"));
        assert!(id.permissions.contains("crm.read"));
        assert!(
            check(&JwtAuth::new(&issuer, "another-api"), access)
                .await
                .is_none(),
            "wrong audience"
        );
        assert!(
            check(
                &JwtAuth::new(&format!("{base}/realms/master"), "zeyn-api"),
                access
            )
            .await
            .is_none(),
            "wrong issuer"
        );
        let token = admin_token(&http, &base).await;
        let _ = http
            .delete(format!("{base}/admin/realms/{REALM}"))
            .bearer_auth(&token)
            .send()
            .await;
    }
}
