//! Accounts: managers, passwords and .NET hash migration, purpose tokens, lockout, sessions with
//! security stamp validation, two factor, recovery codes, bearer tokens, the JSON endpoints and
//! the security rules around them (no user enumeration, rate limits, CSRF, no secrets in output).

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value as Json, json};
use std::time::{Duration, Instant};
use tower::ServiceExt;
use zeyn::accounts::{
    self, AccountEventKind, Accounts, Claim, MemoryAccountEvents, MemoryAccountStore, MemorySender,
    PasswordDenyList, PasswordHasher, Purpose, SignInResult, User, dotnet, normalize,
};
use zeyn::prelude::*;

const KEY: &str = "an accounts key for tests only, not used anywhere real";
const CSRF: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const PASSWORD: &str = "Secret-123";

/// A light Argon2 setting so debug test runs stay fast; the format is the same.
fn light() -> PasswordHasher {
    PasswordHasher {
        memory_kib: 256,
        iterations: 1,
        parallelism: 1,
    }
}

fn accounts_with(sender: MemorySender, events: MemoryAccountEvents) -> Accounts {
    Accounts::new(MemoryAccountStore::new(), Secret::new(KEY.into()))
        .options(|o| {
            o.hasher = light();
            o.ip_attempts = (1000, Duration::from_secs(60));
            o.account_attempts = (1000, Duration::from_secs(60));
        })
        .sender(sender)
        .events(events)
        .public_url("http://localhost:3071")
        .role("admins", ["admin.users"])
}

fn accounts() -> Accounts {
    accounts_with(MemorySender::default(), MemoryAccountEvents::default())
}

#[screen(route = "/home", permission = "admin.users")]
pub fn home() -> Screen {
    Screen::new("Home")
}

/// Who the request is.
#[action(permission = "admin.users")]
pub async fn whoami(_unit: (), ctx: Ctx) -> Result<String> {
    Ok(ctx
        .identity()
        .user
        .clone()
        .unwrap_or_default())
}

/// The tenants of the request's user.
#[action(permission = "admin.users")]
pub async fn my_tenants(_unit: (), ctx: Ctx) -> Result<String> {
    let i = ctx.identity();
    Ok(format!("{:?} {:?}", i.tenant, i.tenants))
}

/// An action that needs a recent sign in.
#[action(permission = "admin.users", reauth)]
pub async fn rotate_keys(_unit: ()) -> Result<()> {
    Ok(())
}

fn app_with(accounts: Accounts, reauth: Duration) -> Router {
    App::new("T")
        .accounts(accounts)
        .reauth_within(reauth)
        .feature(
            Feature::new("crm")
                .screen(home)
                .api(whoami)
                .api(my_tenants)
                .api(rotate_keys),
        )
        .feature(accounts::ui().admin().build())
        .router()
        .unwrap()
}

fn app(accounts: Accounts) -> Router {
    app_with(accounts, Duration::from_secs(300))
}

fn form(screen: &str, params: Json, node: &str, values: Json, cookies: &str) -> Request<Body> {
    post(
        "/_zeyn/form",
        json!({ "screen": screen, "params": params, "node": node, "values": values }),
        cookies,
    )
}

#[tokio::test]
async fn the_sign_in_screen_signs_in_without_a_page_load_and_only_goes_to_local_paths() {
    let a = accounts();
    user(&a, "ui@example.com").await;
    let router = app(a.clone());
    let page = send(&router, get("/account/login", "")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body
            .contains(r#"type="password" autocomplete="current-password""#),
        "password input"
    );
    assert!(
        page.body
            .contains("/_zeyn/account/accounts.js")
    );
    assert!(
        page.body
            .contains(r#"<div class="z-app z-auth" data-zeyn-shell="auth">"#)
            && page.body.contains("/_zeyn/auth.css")
            && !page
                .body
                .contains(r#"<nav class="z-nav""#)
            && !page.body.contains("data-zeyn-commands"),
        "the sign in screens open in the sign in shell, without menu or palette"
    );
    assert!(
        page.body
            .contains(r#"autocomplete="username" autocapitalize="none" spellcheck="false""#),
        "the user name is filled by password managers"
    );
    assert!(page.body.contains(
        r#"<p class="z-caps" data-zeyn-caps aria-live="polite" hidden>Caps Lock is on</p>"#
    ));
    let ok = send(
        &router,
        form(
            "account_login",
            json!({ "return_to": "/home" }),
            "0.0",
            json!({ "user_name": "ui@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    assert!(
        ok.body.contains(r#""effect":"go""#)
            && ok.body.contains(r#""url":"/home""#)
            && ok.body.contains(r#""frame":true"#),
        "{}",
        ok.body
    );
    assert!(
        ok.cookie("zeyn_account")
            .is_some_and(|c| !c.is_empty())
    );
    let evil = send(
        &router,
        form(
            "account_login",
            json!({ "return_to": "//evil.example/x" }),
            "0.0",
            json!({ "user_name": "ui@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert!(
        evil.body.contains(r#""url":"/""#) && !evil.body.contains("evil.example"),
        "{}",
        evil.body
    );
    let wrong = send(
        &router,
        form(
            "account_login",
            json!({}),
            "0.0",
            json!({ "user_name": "ui@example.com", "password": "nope" }),
            "",
        ),
    )
    .await;
    assert!(wrong.cookie("zeyn_account").is_none());
    assert!(wrong.body.contains("wrong"), "{}", wrong.body);
}

#[tokio::test]
async fn manage_screens_need_a_session_and_admin_screens_a_permission() {
    let a = accounts();
    user(&a, "m@example.com").await;
    let router = app(a.clone());
    let out = send(&router, get("/account/manage", "")).await;
    assert!(out.status.is_redirection(), "{}", out.status);
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "m@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());
    for path in [
        "/account/manage",
        "/account/manage/email",
        "/account/manage/password",
        "/account/manage/two-factor",
        "/account/manage/passkeys",
        "/account/manage/logins",
        "/account/manage/personal-data",
    ] {
        let r = send(&router, get(path, &session)).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert!(!r.body.contains("argon2"), "{path}");
    }
    let qr = send(&router, get("/account/manage/two-factor", &session)).await;
    assert!(qr.body.contains("<svg") && qr.body.contains("z-totp-key"));
    let admin = send(&router, get("/account/admin/users", &session)).await;
    assert_ne!(
        admin.status,
        StatusCode::OK,
        "admin screens need accounts.users.read"
    );
}

#[tokio::test]
async fn every_account_screen_renders_in_both_languages() {
    let a = accounts();
    let router = app(a.clone());
    let r = send(&router, get("/account/login?return_to=%2Fhome", "")).await;
    assert!(
        r.body.contains("Sign in")
            && r.body.contains("Password")
            && r.body
                .contains(r#"data-return="/home""#)
    );
    let tr = zeyn::I18n::new("en");
    assert_eq!(
        tr.text("tr", &Text::key("zeyn.accounts.login.title")),
        "Oturum aç"
    );
    assert_eq!(tr.text("tr", &Text::key("AccountLogin.password")), "Parola");
    for path in [
        "/account/register",
        "/account/forgot-password",
        "/account/resend-confirmation",
        "/account/lockout",
        "/account/access-denied",
        "/account/check-email",
        "/account/logout",
        "/account/login-recovery",
        "/account/login-2fa",
        "/account/reset-password",
        "/account/confirm-email",
    ] {
        assert_eq!(
            send(&router, get(path, ""))
                .await
                .status,
            StatusCode::OK,
            "{path}"
        );
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|c| c.starts_with(&format!("{name}=")))
            .map(|c| {
                c[name.len() + 1..]
                    .split(';')
                    .next()
                    .unwrap()
                    .to_string()
            })
    }

    fn json(&self) -> Json {
        serde_json::from_str(&self.body).unwrap_or(Json::Null)
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

fn post(path: &str, body: Json, cookies: &str) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={CSRF}; {cookies}"))
        .header("x-zeyn-csrf", CSRF)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(path: &str, cookies: &str) -> Request<Body> {
    Request::get(path)
        .header(header::COOKIE, cookies)
        .body(Body::empty())
        .unwrap()
}

fn page(cookies: &str) -> Request<Body> {
    get("/home", cookies)
}

/// RFC 6238 code for a base32 key at the current time, as an authenticator app computes it.
fn totp_now(key: &str) -> String {
    let bytes = base32(key);
    let step = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 30)
        .to_be_bytes();
    let tag = ring::hmac::sign(
        &ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &bytes),
        &step,
    );
    let h = tag.as_ref();
    let o = (h[19] & 15) as usize;
    let n = (u32::from(h[o]) & 0x7f) << 24
        | u32::from(h[o + 1]) << 16
        | u32::from(h[o + 2]) << 8
        | u32::from(h[o + 3]);
    format!("{:06}", n % 1_000_000)
}

fn base32(s: &str) -> Vec<u8> {
    let (mut buf, mut bits, mut out) = (0u32, 0u32, Vec::new());
    for c in s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
    {
        let v = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 65,
            c => c as u32 - '2' as u32 + 26,
        };
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

async fn user(accounts: &Accounts, email: &str) -> User {
    accounts
        .users()
        .create(User::new(email).with_email(email), Some(PASSWORD))
        .await
        .unwrap()
}

#[tokio::test]
async fn users_are_created_with_normalized_names_and_stamps() {
    let a = accounts();
    let u = user(&a, "Ada@Example.com").await;
    assert_eq!(u.normalized_user_name, "ADA@EXAMPLE.COM");
    assert!(!u.security_stamp.is_empty() && !u.concurrency_stamp.is_empty() && u.lockout_enabled);
    assert!(
        u.password_hash
            .as_deref()
            .unwrap()
            .starts_with("$argon2id$")
    );
    assert!(
        a.users()
            .find_by_name("ada@example.com")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        a.users()
            .find_by_email("ADA@example.COM")
            .await
            .unwrap()
            .is_some()
    );
    let dup = a
        .users()
        .create(User::new("ada@EXAMPLE.com"), Some(PASSWORD))
        .await
        .unwrap_err();
    assert!(matches!(dup, Error::Validation(_)));
}

#[tokio::test]
async fn turkish_letters_normalize_without_a_language() {
    assert_eq!(normalize("admin"), "ADMIN");
    assert_ne!(normalize("admin"), "ADMİN");
    let a = Accounts::new(MemoryAccountStore::new(), Secret::new(KEY.into())).options(|o| {
        o.hasher = light();
        o.user.allowed_user_name_characters = String::new();
    });
    a.users()
        .create(User::new("ali"), Some(PASSWORD))
        .await
        .unwrap();
    assert!(
        a.users()
            .create(User::new("alı"), Some(PASSWORD))
            .await
            .is_err(),
        "look alike names cannot both exist"
    );
    assert!(
        a.users()
            .find_by_name("ALI")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn password_policy_matches_the_dotnet_defaults() {
    let a = accounts();
    let codes = |pw: &str| {
        a.settings()
            .password
            .check(pw)
            .into_iter()
            .map(|(c, _)| c)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes("abc"),
        [
            "password_too_short",
            "password_requires_non_alphanumeric",
            "password_requires_digit",
            "password_requires_upper"
        ]
    );
    assert!(codes("Secret-123").is_empty());
    let err = a
        .users()
        .create(User::new("x@example.com"), Some("short"))
        .await
        .unwrap_err();
    let Error::Validation(v) = err else { panic!() };
    assert_eq!(v.errors[0].field, "password");
    let b = accounts().breached_passwords(PasswordDenyList::new(["Password-1"]));
    assert!(
        b.users()
            .validate_password("Password-1")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn dotnet_v2_and_v3_hashes_verify_and_are_rehashed_to_argon2id() {
    let a = accounts();
    for (i, stored) in [
        dotnet::hash_v2("Migrated-1", [7; 16]),
        dotnet::hash_v3("Migrated-1", dotnet::Prf::Sha256, 10_000, &[9; 16]),
        dotnet::hash_v3("Migrated-1", dotnet::Prf::Sha512, 100_000, &[3; 16]),
        dotnet::hash_v3("Migrated-1", dotnet::Prf::Sha1, 1000, &[1; 16]),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(dotnet::verify(&stored, "Migrated-1"));
        assert!(!dotnet::verify(&stored, "migrated-1"));
        let email = format!("m{i}@example.com");
        let mut u = a
            .users()
            .create(User::new(&email), None)
            .await
            .unwrap();
        u.password_hash = Some(stored.clone());
        a.users().update(&mut u).await.unwrap();
        assert_eq!(
            a.sign_in()
                .password_sign_in(&email, "Migrated-1", false, true)
                .await
                .unwrap(),
            SignInResult::Succeeded
        );
        let after = a
            .users()
            .find_by_name(&email)
            .await
            .unwrap()
            .unwrap();
        assert!(
            after
                .password_hash
                .unwrap()
                .starts_with("$argon2id$"),
            "rehashed on sign in"
        );
    }
    assert!(
        !dotnet::verify("AQAAAAEAACcQAAAAEA", "x"),
        "a truncated hash fails"
    );
    let huge = {
        let mut b = vec![1u8, 0, 0, 0, 2];
        b.extend_from_slice(&u32::MAX.to_be_bytes());
        b.extend_from_slice(&16u32.to_be_bytes());
        b.extend_from_slice(&[0; 48]);
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(b)
    };
    let started = Instant::now();
    assert!(!dotnet::verify(&huge, "x"));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "huge iteration counts are refused, not run"
    );
}

#[tokio::test]
async fn purpose_tokens_are_bound_to_user_purpose_and_stamp() {
    let a = accounts();
    let mut u = user(&a, "t@example.com").await;
    let other = user(&a, "o@example.com").await;
    let confirm = a
        .users()
        .generate_email_confirmation_token(&u);
    assert!(
        !a.users()
            .verify_token(&u, &Purpose::ResetPassword, &confirm),
        "wrong purpose"
    );
    assert!(
        !a.users()
            .verify_token(&other, &Purpose::ConfirmEmail, &confirm),
        "wrong user"
    );
    let mut tampered = confirm.clone().into_bytes();
    tampered[20] = if tampered[20] == b'A' { b'B' } else { b'A' };
    assert!(!a.users().verify_token(
        &u,
        &Purpose::ConfirmEmail,
        std::str::from_utf8(&tampered).unwrap()
    ));
    a.users()
        .confirm_email(&mut u, &confirm)
        .await
        .unwrap();
    assert!(u.email_confirmed);
    let reset = a
        .users()
        .generate_password_reset_token(&u);
    a.users()
        .change_password(&mut u, PASSWORD, "Changed-456")
        .await
        .unwrap();
    assert!(
        a.users()
            .reset_password(&mut u, &reset, "Another-789")
            .await
            .is_err(),
        "a password change voids reset links"
    );
    let reset = a
        .users()
        .generate_password_reset_token(&u);
    a.users()
        .reset_password(&mut u, &reset, "Another-789")
        .await
        .unwrap();
    assert!(
        a.users()
            .reset_password(&mut u, &reset, "Again-0000x")
            .await
            .is_err(),
        "a reset link works once"
    );
}

#[tokio::test]
async fn purpose_tokens_expire() {
    let a = accounts().options(|o| {
        o.hasher = light();
        o.tokens.token_lifespan = Duration::from_secs(1);
    });
    let u = user(&a, "e@example.com").await;
    let token = a
        .users()
        .generate_email_confirmation_token(&u);
    assert!(
        a.users()
            .verify_token(&u, &Purpose::ConfirmEmail, &token)
    );
    tokio::time::sleep(Duration::from_millis(2100)).await;
    assert!(
        !a.users()
            .verify_token(&u, &Purpose::ConfirmEmail, &token)
    );
}

#[tokio::test]
async fn five_failures_lock_the_account_and_events_are_recorded() {
    let events = MemoryAccountEvents::default();
    let a = accounts_with(MemorySender::default(), events.clone());
    user(&a, "l@example.com").await;
    for _ in 0..4 {
        assert_eq!(
            a.sign_in()
                .password_sign_in("l@example.com", "wrong", false, true)
                .await
                .unwrap(),
            SignInResult::Failed
        );
    }
    assert_eq!(
        a.sign_in()
            .password_sign_in("l@example.com", "wrong", false, true)
            .await
            .unwrap(),
        SignInResult::LockedOut
    );
    assert_eq!(
        a.sign_in()
            .password_sign_in("l@example.com", PASSWORD, false, true)
            .await
            .unwrap(),
        SignInResult::LockedOut,
        "even the right password"
    );
    let mut u = a
        .users()
        .find_by_name("l@example.com")
        .await
        .unwrap()
        .unwrap();
    a.users()
        .set_lockout_end(&mut u, None)
        .await
        .unwrap();
    assert_eq!(
        a.sign_in()
            .password_sign_in("l@example.com", PASSWORD, false, true)
            .await
            .unwrap(),
        SignInResult::Succeeded
    );
    let kinds: Vec<_> = events
        .events()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(kinds.contains(&AccountEventKind::SignInFailed));
    assert!(kinds.contains(&AccountEventKind::LockedOut));
    assert!(kinds.contains(&AccountEventKind::SignedIn));
}

#[tokio::test]
async fn confirmed_email_can_be_required() {
    let a = accounts().options(|o| {
        o.hasher = light();
        o.sign_in.require_confirmed_email = true;
    });
    user(&a, "c@example.com").await;
    assert_eq!(
        a.sign_in()
            .password_sign_in("c@example.com", PASSWORD, false, true)
            .await
            .unwrap(),
        SignInResult::NotAllowed
    );
}

#[tokio::test]
async fn missing_users_take_as_long_as_wrong_passwords() {
    let a = Accounts::new(MemoryAccountStore::new(), Secret::new(KEY.into())).options(|o| {
        o.hasher = PasswordHasher {
            memory_kib: 4096,
            iterations: 2,
            parallelism: 1,
        }
    });
    user(&a, "timing@example.com").await;
    let once = |name: &'static str| {
        let a = a.clone();
        async move {
            let started = Instant::now();
            assert_eq!(
                a.sign_in()
                    .password_sign_in(name, "Wrong-123", false, false)
                    .await
                    .unwrap(),
                SignInResult::Failed
            );
            started.elapsed().as_secs_f64()
        }
    };
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (mut known, mut unknown) = (Vec::new(), Vec::new());
    for round in 0..9 {
        if round % 2 == 0 {
            known.push(once("timing@example.com").await);
            unknown.push(once("nobody@example.com").await);
        } else {
            unknown.push(once("nobody@example.com").await);
            known.push(once("timing@example.com").await);
        }
    }
    let (known, unknown) = (median(known), median(unknown));
    let ratio = known / unknown;
    assert!(
        (0.5..2.0).contains(&ratio),
        "median known {known:.4}s, median unknown {unknown:.4}s"
    );
}

#[tokio::test]
async fn recovery_codes_have_the_dotnet_shape_are_hashed_and_work_once() {
    let a = accounts();
    let u = user(&a, "r@example.com").await;
    let codes = a
        .users()
        .generate_recovery_codes(&u, 10)
        .await
        .unwrap();
    assert_eq!(codes.len(), 10);
    assert!(
        codes
            .iter()
            .all(|c| c.len() == 11 && &c[5..6] == "-")
    );
    let stored = a
        .store()
        .token(&u.id, accounts::INTERNAL_PROVIDER, accounts::RECOVERY_CODES)
        .await
        .unwrap()
        .unwrap();
    assert!(!stored.contains(&codes[0]), "only hashes are stored");
    assert!(
        a.users()
            .redeem_recovery_code(&u, &codes[0].to_lowercase())
            .await
            .unwrap()
    );
    assert!(
        !a.users()
            .redeem_recovery_code(&u, &codes[0])
            .await
            .unwrap()
    );
    assert_eq!(
        a.users()
            .count_recovery_codes(&u)
            .await
            .unwrap(),
        9
    );
}

#[tokio::test]
async fn authenticator_codes_work_once_and_the_key_is_sealed() {
    let a = accounts();
    let mut u = user(&a, "k@example.com").await;
    let key = a
        .users()
        .reset_authenticator_key(&mut u)
        .await
        .unwrap();
    assert_eq!(base32(&key).len(), 20);
    let stored = a
        .store()
        .token(
            &u.id,
            accounts::INTERNAL_PROVIDER,
            accounts::AUTHENTICATOR_KEY,
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!stored.contains(&key));
    let code = totp_now(&key);
    assert!(
        a.users()
            .verify_authenticator_code(&u, &code)
            .await
            .unwrap()
    );
    assert!(
        !a.users()
            .verify_authenticator_code(&u, &code)
            .await
            .unwrap(),
        "replay refused"
    );
    assert!(
        a.users()
            .authenticator_uri(&u, &key)
            .starts_with("otpauth://totp/Zeyn:k%40example.com?secret=")
    );
}

#[tokio::test]
async fn debug_output_and_personal_data_hide_secrets() {
    let a = accounts();
    let u = user(&a, "s@example.com").await;
    let shown = format!("{u:?}");
    assert!(
        !shown.contains(u.password_hash.as_deref().unwrap()) && !shown.contains(&u.security_stamp)
    );
    let data = a
        .users()
        .personal_data(&u)
        .await
        .unwrap()
        .to_string();
    assert!(
        !data.contains("argon2")
            && !data.contains(&u.security_stamp)
            && data.contains("s@example.com")
    );
}

#[tokio::test]
async fn roles_and_role_claims_become_permissions() {
    let a = accounts();
    let mut u = user(&a, "admin@example.com").await;
    a.roles()
        .create("Admins")
        .await
        .unwrap();
    let auditors = a
        .roles()
        .create("auditors")
        .await
        .unwrap();
    a.roles()
        .add_claim(&auditors, Claim::new("permission", "audit.read"))
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "admins")
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "AUDITORS")
        .await
        .unwrap();
    assert_eq!(a.users().roles(&u).await.unwrap(), ["Admins", "auditors"]);
    let router = app(a.clone());
    let r = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "admin@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let session = r.cookie("zeyn_account").unwrap();
    assert!(
        r.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|c| c.to_str().unwrap().contains("Max-Age")),
        "useCookies is persistent"
    );
    let me = send(&router, page(&format!("zeyn_account={session}"))).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert!(me.body.contains("Home"));
}

#[tokio::test]
async fn register_answers_the_same_for_new_and_existing_addresses() {
    let sender = MemorySender::default();
    let a = accounts_with(sender.clone(), MemoryAccountEvents::default());
    let router = app(a.clone());
    let first = send(
        &router,
        post(
            "/_zeyn/account/register",
            json!({ "email": "new@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let again = send(
        &router,
        post(
            "/_zeyn/account/register",
            json!({ "email": "NEW@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert_eq!(
        (first.status, first.body.as_str()),
        (again.status, again.body.as_str())
    );
    let kinds: Vec<String> = sender
        .messages()
        .iter()
        .map(|(_, m)| format!("{m:?}"))
        .collect();
    assert_eq!(
        kinds,
        ["ConfirmEmail([redacted])", "AlreadyRegistered([redacted])"]
    );
    let forgot_known = send(
        &router,
        post(
            "/_zeyn/account/forgotPassword",
            json!({ "email": "new@example.com" }),
            "",
        ),
    )
    .await;
    let forgot_unknown = send(
        &router,
        post(
            "/_zeyn/account/forgotPassword",
            json!({ "email": "ghost@example.com" }),
            "",
        ),
    )
    .await;
    assert_eq!(
        (forgot_known.status, forgot_known.body),
        (forgot_unknown.status, forgot_unknown.body)
    );
    let weak = send(
        &router,
        post(
            "/_zeyn/account/register",
            json!({ "email": "w@example.com", "password": "weak" }),
            "",
        ),
    )
    .await;
    assert_eq!(weak.status, StatusCode::BAD_REQUEST);
    assert!(
        weak.json()["errors"]["PasswordTooShort"].is_array(),
        "{}",
        weak.body
    );
}

#[tokio::test]
async fn confirmation_link_and_password_reset_flow() {
    let sender = MemorySender::default();
    let a = accounts_with(sender.clone(), MemoryAccountEvents::default());
    let router = app(a.clone());
    send(
        &router,
        post(
            "/_zeyn/account/register",
            json!({ "email": "flow@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let link = sender.last("flow@example.com").unwrap();
    assert!(link.starts_with("http://localhost:3071/account/confirm-email?params="));
    let params: Json =
        serde_json::from_str(&urldecode(link.split("params=").nth(1).unwrap())).unwrap();
    let confirm = send(
        &router,
        get(
            &format!(
                "/_zeyn/account/confirmEmail?userId={}&code={}",
                params["user"].as_str().unwrap(),
                params["code"].as_str().unwrap()
            ),
            "",
        ),
    )
    .await;
    assert_eq!(confirm.status, StatusCode::OK);
    send(
        &router,
        post(
            "/_zeyn/account/forgotPassword",
            json!({ "email": "flow@example.com" }),
            "",
        ),
    )
    .await;
    let reset: Json = serde_json::from_str(&urldecode(
        sender
            .last("flow@example.com")
            .unwrap()
            .split("params=")
            .nth(1)
            .unwrap(),
    ))
    .unwrap();
    let r = send(
        &router,
        post(
            "/_zeyn/account/resetPassword",
            json!({ "email": "flow@example.com", "resetCode": reset["code"], "newPassword": "Brand-new-1" }),
            "",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let login = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "flow@example.com", "password": "Brand-new-1" }),
            "",
        ),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    let again = send(
        &router,
        post(
            "/_zeyn/account/resetPassword",
            json!({ "email": "flow@example.com", "resetCode": reset["code"], "newPassword": "Brand-new-2" }),
            "",
        ),
    )
    .await;
    assert_eq!(again.status, StatusCode::BAD_REQUEST);
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

#[tokio::test]
async fn bearer_tokens_refresh_until_the_stamp_changes() {
    let a = accounts();
    let mut u = user(&a, "b@example.com").await;
    a.roles()
        .create("admins")
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "admins")
        .await
        .unwrap();
    let router = app(a.clone());
    let login = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "b@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let t = login.json();
    assert_eq!(
        (t["tokenType"].as_str(), t["expiresIn"].as_i64()),
        (Some("Bearer"), Some(3600))
    );
    assert!(
        login.cookie("zeyn_account").is_none(),
        "bearer logins set no session cookie"
    );
    let call = |token: &str| {
        Request::post("/api/v1/crm/whoami")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("null"))
            .unwrap()
    };
    assert_eq!(
        send(&router, call(t["accessToken"].as_str().unwrap()))
            .await
            .status,
        StatusCode::OK
    );
    let refreshed = send(
        &router,
        post(
            "/_zeyn/account/refresh",
            json!({ "refreshToken": t["refreshToken"] }),
            "",
        ),
    )
    .await;
    assert_eq!(refreshed.status, StatusCode::OK);
    let mut u = a
        .users()
        .find_by_id(&u.id)
        .await
        .unwrap()
        .unwrap();
    a.users()
        .change_password(&mut u, PASSWORD, "Changed-456")
        .await
        .unwrap();
    let refused = send(
        &router,
        post(
            "/_zeyn/account/refresh",
            json!({ "refreshToken": t["refreshToken"] }),
            "",
        ),
    )
    .await;
    assert_eq!(
        refused.status,
        StatusCode::UNAUTHORIZED,
        "a password change voids refresh tokens"
    );
}

fn api_call(path: &str, token: &str) -> Request<Body> {
    Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("null"))
        .unwrap()
}

#[tokio::test]
async fn refreshed_bearer_tokens_keep_the_original_sign_in_time() {
    let a = accounts();
    let mut u = user(&a, "fresh@example.com").await;
    a.roles()
        .create("admins")
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "admins")
        .await
        .unwrap();
    let router = app_with(a, Duration::from_secs(4));
    let login = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "fresh@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let t = login.json();
    assert_eq!(
        send(
            &router,
            api_call(
                "/api/v1/crm/rotate_keys",
                t["accessToken"].as_str().unwrap()
            )
        )
        .await
        .status,
        StatusCode::OK
    );
    tokio::time::sleep(Duration::from_millis(4300)).await;
    let mut refresh = t["refreshToken"].clone();
    let mut access = String::new();
    for _ in 0..2 {
        let r = send(
            &router,
            post(
                "/_zeyn/account/refresh",
                json!({ "refreshToken": refresh }),
                "",
            ),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
        refresh = r.json()["refreshToken"].clone();
        access = r.json()["accessToken"]
            .as_str()
            .unwrap()
            .to_string();
    }
    let r = send(&router, api_call("/api/v1/crm/rotate_keys", &access)).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "a refresh is not a new sign in: {}",
        r.body
    );
    assert!(r.body.contains("reauth_required"), "{}", r.body);
}

#[tokio::test]
async fn tenant_claims_give_account_users_their_tenants() {
    let a = accounts();
    let mut u = user(&a, "multi@example.com").await;
    a.roles()
        .create("admins")
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "admins")
        .await
        .unwrap();
    a.users()
        .add_claim(&mut u, Claim::new("tenant", "acme"))
        .await
        .unwrap();
    a.users()
        .add_claim(&mut u, Claim::new("tenant", "globex"))
        .await
        .unwrap();
    let router = app(a);
    let login = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "multi@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let token = login.json()["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let r = send(&router, api_call("/api/v1/crm/my_tenants", &token)).await;
    assert_eq!(r.json(), json!(r#"Some("acme") {"globex"}"#), "{}", r.body);
}

#[tokio::test]
async fn second_factor_and_passkey_changes_need_a_recent_sign_in() {
    let a = accounts();
    user(&a, "old@example.com").await;
    let router = app_with(a, Duration::ZERO);
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "old@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());
    let status = send(
        &router,
        post("/_zeyn/account/manage/2fa", json!({}), &session),
    )
    .await;
    assert_eq!(status.status, StatusCode::OK, "{}", status.body);
    assert!(
        status.json()["sharedKey"].is_null(),
        "the key is shown only after a recent sign in: {}",
        status.body
    );
    for change in [
        json!({ "resetRecoveryCodes": true }),
        json!({ "enable": false }),
        json!({ "resetSharedKey": true }),
        json!({ "enable": true, "twoFactorCode": "000000" }),
    ] {
        let r = send(
            &router,
            post("/_zeyn/account/manage/2fa", change.clone(), &session),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{change}: {}", r.body);
        assert!(r.body.contains("reauth_required"), "{}", r.body);
    }
    for path in [
        "/_zeyn/account/manage/passkeys/creationOptions",
        "/_zeyn/account/manage/passkeys/remove",
    ] {
        let r = send(
            &router,
            post(path, json!({ "credentialId": "AAAA" }), &session),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{path}: {}", r.body);
    }
    let screen = send(&router, get("/account/manage/two-factor", &session)).await;
    assert_eq!(screen.status, StatusCode::OK, "{}", screen.body);
    assert!(
        !screen.body.contains("z-totp-key") && !screen.body.contains("z-account-qr"),
        "{}",
        screen.body
    );
}

#[tokio::test]
async fn a_changed_security_stamp_signs_other_sessions_out() {
    let a = accounts().options(|o| {
        o.hasher = light();
        o.session.validation_interval = Duration::ZERO;
    });
    let mut u = user(&a, "stamp@example.com").await;
    a.roles()
        .create("admins")
        .await
        .unwrap();
    a.users()
        .add_to_role(&mut u, "admins")
        .await
        .unwrap();
    let router = app(a.clone());
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useSessionCookies=true",
            json!({ "email": "stamp@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = login.cookie("zeyn_account").unwrap();
    assert!(
        !login
            .headers
            .get_all(header::SET_COOKIE)
            .iter()
            .any(|c| c
                .to_str()
                .unwrap()
                .starts_with("zeyn_account=")
                && c.to_str().unwrap().contains("Max-Age")),
        "a browser session cookie"
    );
    let ok = send(&router, page(&format!("zeyn_account={session}"))).await;
    assert_eq!(ok.status, StatusCode::OK);
    let mut u = a
        .users()
        .find_by_id(&u.id)
        .await
        .unwrap()
        .unwrap();
    a.users()
        .update_security_stamp(&mut u)
        .await
        .unwrap();
    let out = send(&router, page(&format!("zeyn_account={session}"))).await;
    assert_ne!(out.status, StatusCode::OK, "{}", out.body);
    assert_eq!(
        out.cookie("zeyn_account").as_deref(),
        Some(""),
        "the stale cookie is deleted"
    );
}

#[tokio::test]
async fn two_factor_sign_in_with_authenticator_and_recovery_codes() {
    let events = MemoryAccountEvents::default();
    let a = accounts_with(MemorySender::default(), events.clone());
    user(&a, "2fa@example.com").await;
    let router = app(a.clone());
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "2fa@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());
    let setup = send(
        &router,
        post("/_zeyn/account/manage/2fa", json!({}), &session),
    )
    .await;
    assert_eq!(setup.status, StatusCode::OK, "{}", setup.body);
    let key = setup.json()["sharedKey"]
        .as_str()
        .unwrap()
        .to_string();
    let wrong = send(
        &router,
        post(
            "/_zeyn/account/manage/2fa",
            json!({ "enable": true, "twoFactorCode": "000000" }),
            &session,
        ),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    let on = send(
        &router,
        post(
            "/_zeyn/account/manage/2fa",
            json!({ "enable": true, "twoFactorCode": totp_now(&key) }),
            &session,
        ),
    )
    .await;
    assert_eq!(on.status, StatusCode::OK, "{}", on.body);
    let codes: Vec<String> = on.json()["recoveryCodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(codes.len(), 10);
    let step = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "2fa@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert_eq!(step.status, StatusCode::UNAUTHORIZED);
    assert!(step.body.contains("RequiresTwoFactor"));
    let with_code = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "2fa@example.com", "password": PASSWORD, "twoFactorRecoveryCode": codes[0] }),
            "",
        ),
    )
    .await;
    assert_eq!(with_code.status, StatusCode::OK, "{}", with_code.body);
    let reused = send(
        &router,
        post(
            "/_zeyn/account/login",
            json!({ "email": "2fa@example.com", "password": PASSWORD, "twoFactorRecoveryCode": codes[0] }),
            "",
        ),
    )
    .await;
    assert_eq!(reused.status, StatusCode::UNAUTHORIZED);
    let kinds: Vec<_> = events
        .events()
        .into_iter()
        .map(|e| e.kind)
        .collect();
    assert!(
        kinds.contains(&AccountEventKind::TwoFactorEnabled)
            && kinds.contains(&AccountEventKind::RecoveryCodeUsed)
    );
}

#[tokio::test]
async fn posts_need_json_and_cookie_sessions_need_the_csrf_token() {
    let a = accounts();
    user(&a, "csrf@example.com").await;
    let router = app(a.clone());
    let form = Request::post("/_zeyn/account/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("email=a&password=b"))
        .unwrap();
    assert_eq!(
        send(&router, form).await.status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "csrf@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = login.cookie("zeyn_account").unwrap();
    let no_token = Request::post("/_zeyn/account/manage/2fa")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_account={session}"))
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&router, no_token).await.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn any_cookie_needs_the_csrf_token_and_a_bearer_token_needs_none() {
    let a = accounts();
    user(&a, "bearer@example.com").await;
    let router = app(a.clone());
    let other_cookie = Request::post("/_zeyn/account/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, "zeyn_theme=dark")
        .body(Body::from(
            json!({ "email": "bearer@example.com", "password": PASSWORD }).to_string(),
        ))
        .unwrap();
    assert_eq!(
        send(&router, other_cookie).await.status,
        StatusCode::FORBIDDEN,
        "a cookie other than the session still needs the CSRF token"
    );
    let no_cookie = Request::post("/_zeyn/account/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "email": "bearer@example.com", "password": PASSWORD }).to_string(),
        ))
        .unwrap();
    let tokens = send(&router, no_cookie).await;
    assert_eq!(tokens.status, StatusCode::OK, "{}", tokens.body);
    let access: Json = serde_json::from_str(&tokens.body).unwrap();
    let access = access["accessToken"]
        .as_str()
        .unwrap()
        .to_string();
    let with_bearer = Request::post("/_zeyn/account/manage/2fa")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {access}"))
        .header(header::COOKIE, "zeyn_theme=dark")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&router, with_bearer).await.status, StatusCode::OK);
}

#[tokio::test]
async fn sign_in_attempts_are_rate_limited_per_account() {
    let a = accounts().options(|o| {
        o.hasher = light();
        o.account_attempts = (3, Duration::from_secs(300));
    });
    user(&a, "rl@example.com").await;
    let router = app(a.clone());
    let mut last = StatusCode::OK;
    for _ in 0..4 {
        last = send(
            &router,
            post(
                "/_zeyn/account/login",
                json!({ "email": "rl@example.com", "password": "Wrong-123" }),
                "",
            ),
        )
        .await
        .status;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn external_logins_link_sign_in_and_never_unlink_the_last_way_in() {
    let a = accounts();
    let mut linked = a
        .users()
        .create(
            User::new("ext@example.com").with_email("ext@example.com"),
            None,
        )
        .await
        .unwrap();
    let login = accounts::UserLogin {
        provider: "keycloak".into(),
        provider_key: "sub-42".into(),
        display_name: None,
    };
    a.users()
        .add_login(&mut linked, login.clone())
        .await
        .unwrap();
    let mut other = user(&a, "other@example.com").await;
    assert!(
        a.users()
            .add_login(&mut other, login)
            .await
            .is_err(),
        "one provider key belongs to one user"
    );
    assert_eq!(
        a.sign_in()
            .external_login_sign_in("keycloak", "sub-42", false)
            .await
            .unwrap(),
        SignInResult::Succeeded
    );
    assert_eq!(
        a.sign_in()
            .external_login_sign_in("keycloak", "sub-unknown", false)
            .await
            .unwrap(),
        SignInResult::Failed
    );
    assert!(
        a.users()
            .remove_login(&mut linked, "keycloak", "sub-42")
            .await
            .is_err(),
        "the last way to sign in stays"
    );
    a.users()
        .add_password(&mut linked, "Another-123")
        .await
        .unwrap();
    a.users()
        .remove_login(&mut linked, "keycloak", "sub-42")
        .await
        .unwrap();
    assert!(
        a.users()
            .find_by_login("keycloak", "sub-42")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn manage_info_never_returns_hashes_or_stamps() {
    let a = accounts();
    let u = user(&a, "info@example.com").await;
    let router = app(a.clone());
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "info@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());
    let info = send(&router, get("/_zeyn/account/manage/info", &session)).await;
    assert_eq!(
        info.json(),
        json!({ "email": "info@example.com", "isEmailConfirmed": false })
    );
    let data = send(&router, get("/_zeyn/account/manage/personalData", &session)).await;
    assert!(
        data.headers
            .get(header::CONTENT_DISPOSITION)
            .is_some()
    );
    assert!(!data.body.contains(&u.security_stamp) && !data.body.contains("argon2"));
}

#[tokio::test]
async fn the_register_screen_goes_to_a_screen_that_opens() {
    let router = app(accounts());
    let r = send(
        &router,
        form(
            "account_register",
            json!(null),
            "0.0",
            json!({ "email": "s@example.com", "password": PASSWORD, "confirm_password": PASSWORD }),
            "",
        ),
    )
    .await;
    assert!(
        r.body
            .contains(r#""url":"/account/check-email""#),
        "{}",
        r.body
    );
    assert_eq!(
        send(&router, get("/account/check-email", ""))
            .await
            .status,
        StatusCode::OK
    );
    let mismatch = send(
        &router,
        form(
            "account_register",
            json!(null),
            "0.0",
            json!({ "email": "t@example.com", "password": PASSWORD, "confirm_password": "Other-123" }),
            "",
        ),
    )
    .await;
    assert!(mismatch.body.contains("do not match"), "{}", mismatch.body);
}

#[tokio::test]
async fn signing_out_needs_a_post_with_the_csrf_token() {
    let a = accounts();
    user(&a, "out@example.com").await;
    let router = app(a.clone());
    let login = send(
        &router,
        post(
            "/_zeyn/account/login?useCookies=true",
            json!({ "email": "out@example.com", "password": PASSWORD }),
            "",
        ),
    )
    .await;
    let session = login.cookie("zeyn_account").unwrap();
    let cookies = format!("zeyn_account={session}");
    let image = send(&router, get("/_zeyn/account/signout", &cookies)).await;
    assert!(
        matches!(
            image.status,
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ),
        "a cross site image tag cannot sign the user out: {}",
        image.status
    );
    assert_eq!(image.cookie("zeyn_account"), None);
    let no_token = Request::post("/_zeyn/account/logout")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookies)
        .body(Body::from("{}"))
        .unwrap();
    let refused = send(&router, no_token).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert_eq!(refused.cookie("zeyn_account"), None);
    let out = send(&router, post("/_zeyn/account/logout", json!({}), &cookies)).await;
    assert_eq!(out.status, StatusCode::OK);
    assert_eq!(out.cookie("zeyn_account").as_deref(), Some(""));
}

/// The per user browser storage key a page carries (`<meta name="zeyn-storage">`).
fn storage_key(page: &str) -> String {
    let start = page
        .find(r#"<meta name="zeyn-storage" content=""#)
        .expect("the page carries a storage key")
        + r#"<meta name="zeyn-storage" content=""#.len();
    page[start..]
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn sign_out_clears_browser_storage_and_pages_key_storage_per_user() {
    let a = accounts();
    let ada = user(&a, "ada@example.com").await;
    let bob = user(&a, "bob@example.com").await;
    let router = app(a.clone());
    let mut keys = Vec::new();
    for u in [&ada, &bob] {
        let email = u.email.clone().unwrap();
        let login = send(
            &router,
            post(
                "/_zeyn/account/login?useCookies=true",
                json!({ "email": email, "password": PASSWORD }),
                "",
            ),
        )
        .await;
        let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());
        let page = send(&router, get("/account/manage", &session)).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let key = storage_key(&page.body);
        assert!(
            !key.is_empty() && !key.contains(&u.id) && !key.contains(&email),
            "the key is opaque: {key}"
        );
        keys.push((key, session));
    }
    assert_ne!(keys[0].0, keys[1].0, "two users get two storage keys");
    let anonymous = send(&router, get("/account/login", "")).await;
    let anonymous = storage_key(&anonymous.body);
    assert!(
        !keys
            .iter()
            .any(|(k, _)| *k == anonymous)
    );
    let again = send(&router, get("/account/manage", &keys[0].1)).await;
    assert_eq!(storage_key(&again.body), keys[0].0, "the key is stable");
    let logout = send(
        &router,
        post("/_zeyn/account/logout", json!({}), &keys[0].1),
    )
    .await;
    assert_eq!(logout.status, StatusCode::OK, "{}", logout.body);
    assert_eq!(
        logout
            .headers
            .get("clear-site-data")
            .map(|v| v.to_str().unwrap()),
        Some(r#""storage""#)
    );
    let page = send(&router, get("/account/login", "")).await;
    assert!(
        page.headers
            .get("clear-site-data")
            .is_none()
    );
}
