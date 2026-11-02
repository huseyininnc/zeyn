//! Wires [`crate::accounts`] into the app: the services, the session authenticator, the request
//! scope with security stamp validation, the JSON endpoints in the shape of ASP.NET Core
//! Identity's `MapIdentityApi`, passkey ceremonies and external logins.
//!
//! The endpoints live under `/_zeyn/account`. Every `POST` must send `content-type:
//! application/json` (a cross site form cannot, and a cross site script cannot without a CORS
//! preflight the app does not grant), and a `POST` that carries the session cookie must also
//! send the `x-zeyn-csrf` token. API and mobile clients use bearer tokens and no cookies.

use super::{App, AppState, cookie, encode};
use crate::accounts::passkey::{self, CEREMONY, Party, PasskeyState};
use crate::accounts::scope::{self, Scope};
use crate::accounts::signin::{AccountsAuth, BEARER, SESSION, Session, bearer};
use crate::accounts::{AccountMessage, Accounts, SignInResult, TwoFactorKind, User, now_secs};
use crate::auth::{b64url, b64url_decode, random};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::{Error, Result};
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::sync::{Arc, Mutex};

/// Base path of the account endpoints.
pub(crate) const BASE: &str = "/_zeyn/account";

impl App {
    /// Turns on the accounts system (see [`crate::accounts`]): registers [`Accounts`],
    /// [`crate::accounts::UserManager`], [`crate::accounts::RoleManager`] and
    /// [`crate::accounts::SignInManager`] as services, signs requests in by the session cookie or
    /// a bearer token, checks security stamps, and serves the JSON endpoints under
    /// `/_zeyn/account`. Add [`crate::accounts::feature`] for the screens.
    pub fn accounts(mut self, accounts: Accounts) -> Self {
        self.setup_problems.extend(accounts.problems());
        self.auth = Some(Arc::new(AccountsAuth(accounts.clone())));
        self.sign_in.accounts = Some(accounts.clone());
        self.service(accounts.clone()).service(accounts.users()).service(accounts.roles()).service(accounts.sign_in())
    }
}

/// Replaces (or removes) one cookie in the request's `Cookie` header.
fn replace_cookie(headers: &mut HeaderMap, name: &str, value: Option<&str>) {
    let mut pairs: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .filter(|p| !p.is_empty() && p.split_once('=').is_none_or(|(k, _)| k != name))
        .map(str::to_string)
        .collect();
    if let Some(v) = value {
        pairs.push(format!("{name}={v}"));
    }
    headers.remove(header::COOKIE);
    if let Ok(v) = HeaderValue::from_str(&pairs.join("; ")) {
        headers.insert(header::COOKIE, v);
    }
}

/// Opens the request scope, and checks the security stamp of the session or bearer token when
/// `validation_interval` has passed: a changed stamp signs the request out, an unchanged one
/// renews the session with fresh roles.
pub(super) async fn layer(State(st): State<Arc<AppState>>, mut req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let Some(accounts) = st.sign_in.accounts.clone() else { return next.run(req).await };
    let peer = req.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip());
    let forwarded = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok());
    let ip = crate::security::client_ip(peer, forwarded, &st.trusted);
    let manager = accounts.sign_in();
    let mut out = Vec::new();
    let secure = st.secure_cookies || accounts.0.public_url.as_deref().is_some_and(|u| u.starts_with("https://"));
    let interval = accounts.settings().session.validation_interval.as_secs() as i64;
    if let Some(value) = cookie(req.headers(), SESSION).map(str::to_string)
        && let Some(s) = manager.open_session(&value)
        && now_secs() - s.validated >= interval
    {
        match manager.revalidate(&s).await {
            Ok(Some(renewed)) => match manager.seal_session(&renewed) {
                Ok(v) => {
                    let age = renewed.persistent.then(|| format!("; Max-Age={}", renewed.expires - now_secs())).unwrap_or_default();
                    out.push(format!("{SESSION}={v}; Path={}; HttpOnly; SameSite=Lax{age}{}", st.cookie_path(), if secure { "; Secure" } else { "" }));
                    replace_cookie(req.headers_mut(), SESSION, Some(&v));
                }
                Err(e) => tracing::warn!("account session not renewed: {e}"),
            },
            Ok(None) => {
                tracing::info!(user = %s.id, "security stamp changed, signing the session out");
                out.push(format!("{SESSION}=; Path={}; HttpOnly; SameSite=Lax; Max-Age=0{}", st.cookie_path(), if secure { "; Secure" } else { "" }));
                replace_cookie(req.headers_mut(), SESSION, None);
            }
            Err(e) => tracing::warn!("security stamp not checked, keeping the session: {e}"),
        }
    }
    if let Some(token) = bearer(req.headers()).map(str::to_string)
        && let Some(s) = accounts.0.sealer.as_ref().and_then(|k| k.open::<Session>(BEARER, &token))
        && now_secs() - s.validated >= interval
        && matches!(manager.validate_security_stamp(&s.id, &s.stamp).await, Ok(None))
    {
        req.headers_mut().remove(header::AUTHORIZATION);
    }
    let cookies = req.headers().get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).collect::<Vec<_>>().join("; ");
    let scope = Arc::new(Scope {
        cookies,
        out: Mutex::new(out),
        ip,
        secure,
        path: st.cookie_path().to_string(),
        prefix: st.prefix.clone(),
        rate: st.rate_store.clone(),
    });
    let mut response = scope::run(scope.clone(), next.run(req)).await;
    for c in scope::taken(&scope) {
        if let Ok(v) = HeaderValue::from_str(&c) {
            response.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    response
}

pub(super) fn routes(router: Router<Arc<AppState>>, st: &Arc<AppState>) -> Router<Arc<AppState>> {
    if st.sign_in.accounts.is_none() {
        return router;
    }
    let router = router
        .route(&format!("{BASE}/register"), post(register))
        .route(&format!("{BASE}/login"), post(login))
        .route(&format!("{BASE}/logout"), post(logout))
        .route(&format!("{BASE}/signout"), get(signout))
        .route(&format!("{BASE}/refresh"), post(refresh))
        .route(&format!("{BASE}/confirmEmail"), get(confirm_email))
        .route(&format!("{BASE}/resendConfirmationEmail"), post(resend))
        .route(&format!("{BASE}/forgotPassword"), post(forgot))
        .route(&format!("{BASE}/resetPassword"), post(reset))
        .route(&format!("{BASE}/manage/2fa"), post(two_factor))
        .route(&format!("{BASE}/manage/info"), get(info).post(update_info))
        .route(&format!("{BASE}/manage/personalData"), get(personal_data))
        .route(&format!("{BASE}/passkeys/requestOptions"), post(passkey_request))
        .route(&format!("{BASE}/passkeys/login"), post(passkey_login))
        .route(&format!("{BASE}/manage/passkeys/creationOptions"), post(passkey_creation))
        .route(&format!("{BASE}/manage/passkeys"), get(passkey_list).post(passkey_add))
        .route(&format!("{BASE}/manage/passkeys/remove"), post(passkey_remove))
        .route(&format!("{BASE}/accounts.js"), get(script));
    #[cfg(feature = "oidc")]
    let router = router.route(&format!("{BASE}/external/{{provider}}"), get(external::start)).route(&format!("{BASE}/external/{{provider}}/callback"), get(external::callback));
    router
}

/// The browser side of passkeys: calls `navigator.credentials` for buttons marked
/// `data-zeyn-passkey` and posts the result. A separate asset, as the CSP requires.
const SCRIPT: &str = include_str!("../../assets/accounts.js");

async fn script() -> Response {
    ([(header::CONTENT_TYPE, "text/javascript; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], SCRIPT).into_response()
}

fn accounts(st: &AppState) -> Accounts {
    st.sign_in.accounts.clone().expect("routes exist only with accounts")
}

/// A JSON body, after the content type and, for cookie sessions, CSRF checks.
fn body<T: serde::de::DeserializeOwned>(st: &AppState, headers: &HeaderMap, raw: &[u8]) -> std::result::Result<T, Response> {
    let json = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("application/json"));
    if !json {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response());
    }
    if cookie(headers, SESSION).is_some() && st.check_csrf(headers).is_err() {
        return Err(problem(StatusCode::FORBIDDEN, "csrf", "The CSRF token is missing or wrong."));
    }
    if raw.len() > 64 * 1024 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE.into_response());
    }
    serde_json::from_slice(raw).map_err(|_| problem(StatusCode::BAD_REQUEST, "bad_request", "The body is not valid JSON for this endpoint."))
}

fn problem(status: StatusCode, code: &str, detail: &str) -> Response {
    let mut r = (status, axum::Json(json!({ "type": format!("urn:zeyn:problem:{code}"), "title": status.canonical_reason().unwrap_or(code), "status": status.as_u16(), "detail": detail }))).into_response();
    r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
    r
}

/// An error as `MapIdentityApi` reports it: validation errors keyed by their code in Pascal
/// case (`PasswordTooShort`), refusals as problems.
fn failed(ctx: &Ctx, e: Error) -> Response {
    let code_of = |t: &Text| match t {
        Text::Key { key, .. } => key.rsplit('.').next().unwrap_or("Error").split('_').map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        }).collect::<String>(),
        _ => "Error".into(),
    };
    match e {
        Error::Validation(v) => {
            let mut errors = serde_json::Map::new();
            for f in &v.errors {
                let entry = errors.entry(code_of(&f.message)).or_insert_with(|| Json::Array(Vec::new()));
                if let Json::Array(list) = entry {
                    list.push(Json::String(ctx.t(&f.message)));
                }
            }
            let mut r = (StatusCode::BAD_REQUEST, axum::Json(json!({ "type": "https://tools.ietf.org/html/rfc9110#section-15.5.1", "title": "One or more validation errors occurred.", "status": 400, "errors": errors }))).into_response();
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
            r
        }
        Error::Denied(t) => {
            let status = if code_of(&t) == "RateLimited" { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::BAD_REQUEST };
            problem(status, &code_of(&t), &ctx.t(&t))
        }
        Error::Internal(detail) => {
            tracing::error!("accounts: {detail}");
            problem(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong.")
        }
        other => problem(StatusCode::BAD_REQUEST, other.code(), &ctx.t(&other.message())),
    }
}

/// The signed in user of the request, by session cookie or bearer token.
pub(crate) async fn current(accounts: &Accounts, headers: &HeaderMap) -> Result<Option<User>> {
    let manager = accounts.sign_in();
    let session = match bearer(headers) {
        Some(t) => accounts.0.sealer.as_ref().and_then(|k| k.open::<Session>(BEARER, t)).filter(|s| s.expires > now_secs()),
        None => cookie(headers, SESSION).and_then(|v| manager.open_session(v)),
    };
    match session {
        Some(s) => accounts.users().find_by_id(&s.id).await,
        None => Ok(None),
    }
}

/// Link of the default UI's screen `path` with params.
pub(crate) fn screen_link(accounts: &Accounts, path: &str, params: &Json) -> String {
    accounts.link(&format!("{path}?params={}", encode(&params.to_string())))
}

/// Sends the e-mail confirmation link (or the e-mail change link to the new address).
pub(crate) async fn send_confirmation(ctx: &Ctx, accounts: &Accounts, user: &User, change_to: Option<&str>) -> Result<()> {
    let users = accounts.users();
    match change_to {
        Some(new) => {
            let code = users.generate_change_email_token(user, new);
            let link = screen_link(accounts, "/account/confirm-email", &json!({ "user": user.id, "code": code, "email": new }));
            accounts.0.sender.send(ctx, new, AccountMessage::ChangeEmail { link }).await
        }
        None => {
            let Some(to) = user.email.clone() else { return Ok(()) };
            let code = users.generate_email_confirmation_token(user);
            let link = screen_link(accounts, "/account/confirm-email", &json!({ "user": user.id, "code": code }));
            accounts.0.sender.send(ctx, &to, AccountMessage::ConfirmEmail { link }).await
        }
    }
}

/// Registers with an e-mail address and password. The answer does not say whether the address
/// had an account: an existing address gets a notice by e-mail instead, after the same password
/// hashing work.
pub(crate) async fn register_user(ctx: &Ctx, accounts: &Accounts, email: &str, password: &str) -> Result<()> {
    if !accounts.settings().registration {
        return Err(Error::NotFound);
    }
    let users = accounts.users();
    accounts.sign_in().throttle_ip().await?;
    users.validate_password(password).await?;
    let email = email.trim();
    if users.find_by_name_or_email(email).await?.is_some() {
        let _ = accounts.settings().hasher.hash(password);
        let link = accounts.link("/account/login");
        accounts.0.sender.send(ctx, email, AccountMessage::AlreadyRegistered { link }).await?;
        return Ok(());
    }
    let user = match users.create(User::new(email).with_email(email), Some(password)).await {
        Err(Error::Validation(v)) if v.errors.iter().any(|e| e.field == "user_name") => return Err(crate::accounts::invalid("email", "invalid_email")),
        r => r?,
    };
    accounts.event(crate::accounts::AccountEventKind::Registered, Some(&user.id));
    send_confirmation(ctx, accounts, &user, None).await
}

/// Starts a password reset: sends a link when the address belongs to a user with a confirmed
/// address, and answers the same in every case.
pub(crate) async fn forgot_password(ctx: &Ctx, accounts: &Accounts, email: &str) -> Result<()> {
    accounts.sign_in().throttle_ip().await?;
    if let Some(user) = accounts.users().find_by_email(email).await?
        && user.email_confirmed
        && let Some(to) = user.email.clone()
    {
        let code = accounts.users().generate_password_reset_token(&user);
        let link = screen_link(accounts, "/account/reset-password", &json!({ "user": user.id, "code": code }));
        accounts.0.sender.send(ctx, &to, AccountMessage::ResetPassword { link }).await?;
    }
    Ok(())
}

/// Sends the confirmation link again when the address belongs to an unconfirmed user; answers
/// the same in every case.
pub(crate) async fn resend_confirmation(ctx: &Ctx, accounts: &Accounts, email: &str) -> Result<()> {
    accounts.sign_in().throttle_ip().await?;
    if let Some(user) = accounts.users().find_by_email(email).await?
        && !user.email_confirmed
    {
        send_confirmation(ctx, accounts, &user, None).await?;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequest {
    email: String,
    password: String,
}

async fn register(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: RegisterRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    match register_user(&ctx, &accounts(&st), &r.email, &r.password).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(Error::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => failed(&ctx, e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    email: String,
    password: String,
    two_factor_code: Option<String>,
    two_factor_recovery_code: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CookieQuery {
    use_cookies: Option<bool>,
    use_session_cookies: Option<bool>,
}

impl CookieQuery {
    /// (cookies, persistent) as `MapIdentityApi` reads the two flags.
    fn mode(&self) -> (bool, bool) {
        let cookies = self.use_cookies == Some(true) || self.use_session_cookies == Some(true);
        (cookies, self.use_cookies == Some(true) && self.use_session_cookies != Some(true))
    }
}

/// Finishes a successful sign in for bearer clients: no cookie, tokens in the body.
async fn tokens_for(accounts: &Accounts, ctx: &Ctx, user_id: &str) -> Response {
    let manager = accounts.sign_in();
    let s = scope::cookie(SESSION).and_then(|v| manager.open_session(&v));
    scope::forget(SESSION);
    let Ok(Some(user)) = accounts.users().find_by_id(user_id).await else { return failed(ctx, Error::Internal("signed in user vanished".into())) };
    let (method, mfa) = s.map(|s| (s.method, s.mfa)).unwrap_or(("pwd".into(), false));
    match manager.bearer(&user, &method, mfa).await {
        Ok(t) => axum::Json(t).into_response(),
        Err(e) => failed(ctx, e),
    }
}

async fn login(State(st): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<CookieQuery>, raw: axum::body::Bytes) -> Response {
    let r: LoginRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let manager = accounts.sign_in();
    let (cookies, persistent) = q.mode();
    let mut result = match manager.password_sign_in(&r.email, &r.password, persistent, true).await {
        Ok(r) => r,
        Err(e) => return failed(&ctx, e),
    };
    if result == SignInResult::RequiresTwoFactor {
        let step = if let Some(code) = r.two_factor_code.as_deref().filter(|c| !c.is_empty()) {
            Some(manager.two_factor_sign_in(TwoFactorKind::Authenticator, code, persistent).await)
        } else if let Some(code) = r.two_factor_recovery_code.as_deref().filter(|c| !c.is_empty()) {
            Some(manager.recovery_code_sign_in(code).await)
        } else {
            None
        };
        if let Some(step) = step {
            result = match step {
                Ok(r) => r,
                Err(e) => return failed(&ctx, e),
            };
        }
    }
    if result != SignInResult::Succeeded {
        let name = match result {
            SignInResult::LockedOut => "LockedOut",
            SignInResult::NotAllowed => "NotAllowed",
            SignInResult::RequiresTwoFactor => "RequiresTwoFactor",
            _ => "Failed",
        };
        return problem(StatusCode::UNAUTHORIZED, name, name);
    }
    if cookies {
        return StatusCode::OK.into_response();
    }
    let Some(id) = scope::cookie(SESSION).and_then(|v| manager.open_session(&v)).map(|s| s.id) else { return failed(&ctx, Error::Internal("no session after sign in".into())) };
    tokens_for(&accounts, &ctx, &id).await
}

async fn logout(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if cookie(&headers, SESSION).is_some() && st.check_csrf(&headers).is_err() {
        return problem(StatusCode::FORBIDDEN, "csrf", "The CSRF token is missing or wrong.");
    }
    let accounts = accounts(&st);
    let user = current(&accounts, &headers).await.ok().flatten();
    accounts.sign_in().sign_out(user.as_ref().map(|u| u.id.as_str()));
    StatusCode::OK.into_response()
}

/// Ends the browser session and goes to the sign in page; the idle timeout sends users here.
async fn signout(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let accounts = accounts(&st);
    let user = current(&accounts, &headers).await.ok().flatten();
    accounts.sign_in().sign_out(user.as_ref().map(|u| u.id.as_str()));
    let mut r = Redirect::to(&st.login_url.clone().unwrap_or_else(|| st.url("/"))).into_response();
    r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: RefreshRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    match accounts(&st).sign_in().refresh_bearer(&r.refresh_token).await {
        Ok(Some(t)) => axum::Json(t).into_response(),
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => failed(&ctx, e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmQuery {
    user_id: String,
    code: String,
    changed_email: Option<String>,
}

/// Confirms an e-mail address (or an e-mail change) from a link, for API clients.
async fn confirm_email(State(st): State<Arc<AppState>>, Query(q): Query<ConfirmQuery>) -> Response {
    let accounts = accounts(&st);
    let users = accounts.users();
    let Ok(Some(mut user)) = users.find_by_id(&q.user_id).await else { return StatusCode::UNAUTHORIZED.into_response() };
    let done = match &q.changed_email {
        None => users.confirm_email(&mut user, &q.code).await,
        Some(new) => users.change_email(&mut user, new, &q.code, true).await,
    };
    match done {
        Ok(()) => (StatusCode::OK, "Thank you for confirming your email.").into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

#[derive(Deserialize)]
struct EmailRequest {
    email: String,
}

async fn resend(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: EmailRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    match resend_confirmation(&ctx, &accounts(&st), &r.email).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => failed(&ctx, e),
    }
}

async fn forgot(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: EmailRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    match forgot_password(&ctx, &accounts(&st), &r.email).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => failed(&ctx, e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetRequest {
    email: String,
    reset_code: String,
    new_password: String,
}

async fn reset(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: ResetRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    if let Err(e) = accounts.sign_in().throttle_ip().await {
        return failed(&ctx, e);
    }
    let users = accounts.users();
    let user = users.find_by_email(&r.email).await.ok().flatten().filter(|u| u.email_confirmed);
    let Some(mut user) = user else { return failed(&ctx, crate::accounts::invalid("code", "invalid_token")) };
    match users.reset_password(&mut user, &r.reset_code, &r.new_password).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(Error::Denied(_)) => failed(&ctx, crate::accounts::invalid("code", "invalid_token")),
        Err(e) => failed(&ctx, e),
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct TwoFactorRequest {
    enable: Option<bool>,
    two_factor_code: Option<String>,
    reset_shared_key: bool,
    reset_recovery_codes: bool,
    forget_machine: bool,
}

async fn two_factor(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: TwoFactorRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let Ok(Some(mut user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    let users = accounts.users();
    let manager = accounts.sign_in();
    let result: Result<Json> = async {
        if r.enable == Some(true) {
            if r.reset_shared_key {
                return Err(crate::accounts::invalid("two_factor_code", "cannot_reset_shared_key_and_enable"));
            }
            let code = r.two_factor_code.as_deref().filter(|c| !c.is_empty()).ok_or_else(|| crate::accounts::invalid("two_factor_code", "requires_two_factor"))?;
            if !users.verify_authenticator_code(&user, code).await? {
                return Err(crate::accounts::invalid("two_factor_code", "invalid_two_factor_code"));
            }
            users.set_two_factor_enabled(&mut user, true).await?;
        } else if r.enable == Some(false) || r.reset_shared_key {
            users.set_two_factor_enabled(&mut user, false).await?;
        }
        if r.reset_shared_key {
            users.reset_authenticator_key(&mut user).await?;
        }
        let mut codes = None;
        if r.reset_recovery_codes || (r.enable == Some(true) && users.count_recovery_codes(&user).await? == 0) {
            codes = Some(users.generate_recovery_codes(&user, 10).await?);
        }
        if r.forget_machine {
            manager.forget_two_factor_client();
        }
        let key = match users.authenticator_key(&user).await? {
            Some(k) => k,
            None => users.reset_authenticator_key(&mut user).await?,
        };
        manager.refresh_sign_in(&user).await?;
        Ok(json!({
            "sharedKey": key,
            "recoveryCodesLeft": match &codes { Some(c) => c.len(), None => users.count_recovery_codes(&user).await? },
            "recoveryCodes": codes,
            "isTwoFactorEnabled": user.two_factor_enabled,
            "isMachineRemembered": manager.is_two_factor_client_remembered(&user),
        }))
    }
    .await;
    match result {
        Ok(j) => axum::Json(j).into_response(),
        Err(e) => failed(&ctx, e),
    }
}

fn info_json(user: &User) -> Json {
    json!({ "email": user.email.as_deref().unwrap_or(&user.user_name), "isEmailConfirmed": user.email_confirmed })
}

async fn info(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    match current(&accounts(&st), &headers).await {
        Ok(Some(user)) => axum::Json(info_json(&user)).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct InfoRequest {
    new_email: Option<String>,
    new_password: Option<String>,
    old_password: Option<String>,
}

async fn update_info(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: InfoRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let Ok(Some(mut user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    let result: Result<()> = async {
        if let Some(new) = r.new_password.as_deref().filter(|p| !p.is_empty()) {
            let old = r.old_password.as_deref().filter(|p| !p.is_empty()).ok_or_else(|| crate::accounts::invalid("old_password", "old_password_required"))?;
            accounts.users().change_password(&mut user, old, new).await?;
            accounts.sign_in().refresh_sign_in(&user).await?;
        }
        if let Some(new) = r.new_email.as_deref().filter(|e| !e.is_empty())
            && Some(new) != user.email.as_deref()
        {
            send_confirmation(&ctx, &accounts, &user, Some(new)).await?;
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => axum::Json(info_json(&user)).into_response(),
        Err(e) => failed(&ctx, e),
    }
}

/// The signed in user's personal data as a JSON download.
async fn personal_data(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let accounts = accounts(&st);
    let Ok(Some(user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    match accounts.users().personal_data(&user).await {
        Ok(data) => {
            tracing::info!(target: "zeyn::accounts", user = %user.id, "personal data downloaded");
            let mut r = axum::Json(data).into_response();
            r.headers_mut().insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"PersonalData.json\""));
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        Err(e) => failed(&st.ctx(&headers), e),
    }
}

/// The relying party of the app, from the accounts public URL.
fn party(accounts: &Accounts, st: &AppState) -> Option<Party> {
    let o = &accounts.settings().passkeys;
    if !o.enabled {
        return None;
    }
    let url = accounts.0.public_url.as_deref()?;
    let name = st.name.english();
    Party::from_url(&format!("{url}{}", st.prefix), o.server_domain.as_deref(), &name)
}

fn start_ceremony(accounts: &Accounts, op: &str, user: Option<String>) -> Result<String> {
    let size = accounts.settings().passkeys.challenge_size.clamp(16, 64);
    let mut challenge = random::<64>().to_vec();
    challenge.truncate(size);
    let state = PasskeyState { op: op.into(), challenge: b64url(&challenge), user, expires: now_secs() + accounts.settings().passkeys.timeout.as_secs() as i64 };
    let sealer = accounts.0.sealer.as_ref().ok_or_else(|| Error::Internal("accounts key too short".into()))?;
    scope::set_cookie(CEREMONY, Some(&sealer.seal(CEREMONY, &state)), Some(accounts.settings().passkeys.timeout.as_secs() as i64));
    Ok(state.challenge)
}

fn take_ceremony(accounts: &Accounts, op: &str) -> Option<PasskeyState> {
    let value = scope::cookie(CEREMONY)?;
    scope::set_cookie(CEREMONY, None, None);
    let s: PasskeyState = accounts.0.sealer.as_ref()?.open(CEREMONY, &value)?;
    (s.op == op && s.expires > now_secs()).then_some(s)
}

/// Options for signing in with a passkey; the browser offers the site's passkeys.
pub(crate) fn passkey_request_options(accounts: &Accounts, st: &AppState) -> Result<Json> {
    let party = party(accounts, st).ok_or(Error::NotFound)?;
    let challenge = start_ceremony(accounts, "get", None)?;
    Ok(passkey::request_options(&accounts.settings().passkeys, &party, &challenge, &[]))
}

async fn passkey_request(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let accounts = accounts(&st);
    if let Err(e) = accounts.sign_in().throttle_ip().await {
        return failed(&st.ctx(&headers), e);
    }
    match passkey_request_options(&accounts, &st) {
        Ok(j) => axum::Json(j).into_response(),
        Err(e) => failed(&st.ctx(&headers), e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasskeyRequest {
    credential_json: String,
    name: Option<String>,
    #[serde(default)]
    remember: bool,
}

/// Verifies a passkey sign in and signs the user in. Returns the user's id.
pub(crate) async fn passkey_sign_in(accounts: &Accounts, st: &AppState, credential: &str, remember: bool) -> Result<(SignInResult, String)> {
    let party = party(accounts, st).ok_or(Error::NotFound)?;
    let state = take_ceremony(accounts, "get").ok_or_else(|| crate::accounts::denied("passkey_expired"))?;
    let credential: Json = serde_json::from_str(credential).map_err(|_| crate::accounts::denied("passkey_invalid"))?;
    let id = passkey::assertion_id(&credential)?;
    let (user_id, stored) = accounts.store().passkey(&id).await?.ok_or_else(|| crate::accounts::denied("passkey_invalid"))?;
    let updated = passkey::verify_assertion(&accounts.settings().passkeys, &party, &state, &credential, &user_id, &stored)?;
    accounts.store().save_passkey(&user_id, &updated).await?;
    let user = accounts.users().find_by_id(&user_id).await?.ok_or_else(|| crate::accounts::denied("passkey_invalid"))?;
    Ok((accounts.sign_in().passkey_sign_in(&user, updated.user_verified, remember).await?, user.id))
}

async fn passkey_login(State(st): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<CookieQuery>, raw: axum::body::Bytes) -> Response {
    let r: PasskeyRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let (cookies, persistent) = q.mode();
    match passkey_sign_in(&accounts, &st, &r.credential_json, persistent || r.remember).await {
        Ok((SignInResult::Succeeded, id)) if !cookies && cookie(&headers, "zeyn_csrf").is_none() => tokens_for(&accounts, &ctx, &id).await,
        Ok((SignInResult::Succeeded, _)) => StatusCode::OK.into_response(),
        Ok((other, _)) => problem(StatusCode::UNAUTHORIZED, &format!("{other:?}"), &format!("{other:?}")),
        Err(e) => failed(&ctx, e),
    }
}

/// Options for adding a passkey to the signed in user.
pub(crate) async fn passkey_creation_options(accounts: &Accounts, st: &AppState, user: &User) -> Result<Json> {
    let party = party(accounts, st).ok_or(Error::NotFound)?;
    let existing = accounts.users().passkeys(user).await?;
    let challenge = start_ceremony(accounts, "create", Some(user.id.clone()))?;
    Ok(passkey::creation_options(&accounts.settings().passkeys, &party, &user.id, &user.user_name, &challenge, &existing))
}

async fn passkey_creation(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if cookie(&headers, SESSION).is_some() && st.check_csrf(&headers).is_err() {
        return problem(StatusCode::FORBIDDEN, "csrf", "The CSRF token is missing or wrong.");
    }
    let accounts = accounts(&st);
    let Ok(Some(user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    match passkey_creation_options(&accounts, &st, &user).await {
        Ok(j) => axum::Json(j).into_response(),
        Err(e) => failed(&st.ctx(&headers), e),
    }
}

/// Verifies and stores a new passkey of the signed in user.
pub(crate) async fn passkey_register(accounts: &Accounts, st: &AppState, user: &mut User, credential: &str, name: Option<String>) -> Result<crate::accounts::Passkey> {
    let party = party(accounts, st).ok_or(Error::NotFound)?;
    let state = take_ceremony(accounts, "create").filter(|s| s.user.as_deref() == Some(user.id.as_str())).ok_or_else(|| crate::accounts::denied("passkey_expired"))?;
    let credential: Json = serde_json::from_str(credential).map_err(|_| crate::accounts::denied("passkey_invalid"))?;
    let pk = passkey::verify_registration(&accounts.settings().passkeys, &party, &state, &credential, name, now_secs())?;
    if accounts.store().passkey(&pk.credential_id).await?.is_some() {
        return Err(crate::accounts::denied("passkey_registered"));
    }
    accounts.store().save_passkey(&user.id, &pk).await?;
    accounts.users().update_security_stamp(user).await?;
    accounts.sign_in().refresh_sign_in(user).await?;
    accounts.event(crate::accounts::AccountEventKind::PasskeyAdded, Some(&user.id));
    Ok(pk)
}

fn passkey_json(p: &crate::accounts::Passkey) -> Json {
    json!({ "credentialId": b64url(&p.credential_id), "name": p.name, "createdAt": p.created_at })
}

async fn passkey_add(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: PasskeyRequest = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let Ok(Some(mut user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    match passkey_register(&accounts, &st, &mut user, &r.credential_json, r.name).await {
        Ok(p) => axum::Json(passkey_json(&p)).into_response(),
        Err(e) => failed(&ctx, e),
    }
}

async fn passkey_list(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let accounts = accounts(&st);
    let Ok(Some(user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    match accounts.users().passkeys(&user).await {
        Ok(list) => axum::Json(list.iter().map(passkey_json).collect::<Vec<_>>()).into_response(),
        Err(e) => failed(&st.ctx(&headers), e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemovePasskey {
    credential_id: String,
}

async fn passkey_remove(State(st): State<Arc<AppState>>, headers: HeaderMap, raw: axum::body::Bytes) -> Response {
    let r: RemovePasskey = match body(&st, &headers, &raw) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let ctx = st.ctx(&headers);
    let accounts = accounts(&st);
    let Ok(Some(mut user)) = current(&accounts, &headers).await else { return StatusCode::NOT_FOUND.into_response() };
    let Some(id) = b64url_decode(r.credential_id.trim_end_matches('=')) else { return failed(&ctx, crate::accounts::invalid("credential_id", "passkey_invalid")) };
    match accounts.users().remove_passkey(&mut user, &id).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => failed(&ctx, e),
    }
}

/// External logins through [`crate::OidcAuth`] providers registered with
/// `Accounts::external`.
#[cfg(feature = "oidc")]
mod external {
    use super::*;
    use crate::auth::oidc::Flow;
    use crate::auth::safe_return;
    use axum::extract::Path;

    /// Cookie of a flow in progress, per provider.
    fn flow_cookie(provider: &str) -> String {
        format!("zeyn_account_ext_{provider}")
    }

    /// Cookie that carries a first external sign in to the screen that creates the account.
    pub(crate) const PENDING: &str = "zeyn_account_external";

    #[derive(Deserialize)]
    pub(crate) struct StartQuery {
        return_to: Option<String>,
        link: Option<String>,
        remember: Option<String>,
    }

    fn redirect_uri(accounts: &Accounts, st: &AppState, provider: &str) -> Option<String> {
        Some(format!("{}{}{BASE}/external/{provider}/callback", accounts.0.public_url.as_deref()?, st.prefix))
    }

    pub(super) async fn start(State(st): State<Arc<AppState>>, Path(provider): Path<String>, Query(q): Query<StartQuery>) -> Response {
        let accounts = accounts(&st);
        let Some(oidc) = accounts.external_provider(&provider) else { return StatusCode::NOT_FOUND.into_response() };
        let (Some(sealer), Some(redirect)) = (oidc.sealer.as_ref(), redirect_uri(&accounts, &st, &provider)) else { return StatusCode::NOT_FOUND.into_response() };
        let back = q.return_to.as_deref().and_then(safe_return).map(str::to_string).unwrap_or_else(|| st.url("/"));
        let flow = Flow {
            state: b64url(&random::<32>()),
            nonce: b64url(&random::<32>()),
            verifier: b64url(&random::<32>()),
            return_to: format!("{}{}{back}", if q.link.is_some() { "L" } else { "S" }, if q.remember.is_some() { "R" } else { "-" }),
            mfa: false,
            expires: now_secs() + 600,
        };
        match oidc.authorize_url(&flow, &redirect, false).await {
            Ok(url) => {
                scope::set_cookie(&flow_cookie(&provider), Some(&sealer.seal(&flow_cookie(&provider), &flow)), Some(600));
                Redirect::to(&url).into_response()
            }
            Err(e) => {
                tracing::error!("external sign in is not available: {e}");
                StatusCode::BAD_GATEWAY.into_response()
            }
        }
    }

    #[derive(Deserialize)]
    pub(crate) struct CallbackQuery {
        code: Option<String>,
        state: Option<String>,
    }

    /// What the pending external sign in cookie holds.
    #[derive(serde::Serialize, Deserialize)]
    pub(crate) struct External {
        #[serde(rename = "p")]
        pub provider: String,
        #[serde(rename = "k")]
        pub key: String,
        #[serde(rename = "m", default)]
        pub email: Option<String>,
        #[serde(rename = "v", default)]
        pub verified: bool,
        #[serde(rename = "n", default)]
        pub name: Option<String>,
        #[serde(rename = "r")]
        pub return_to: String,
        #[serde(rename = "e")]
        pub expires: i64,
    }

    pub(super) async fn callback(State(st): State<Arc<AppState>>, headers: HeaderMap, Path(provider): Path<String>, Query(q): Query<CallbackQuery>) -> Response {
        let accounts = accounts(&st);
        let Some(oidc) = accounts.external_provider(&provider) else { return StatusCode::NOT_FOUND.into_response() };
        let name = flow_cookie(&provider);
        let flow = scope::cookie(&name).and_then(|v| oidc.sealer.as_ref()?.open::<Flow>(&name, &v));
        scope::set_cookie(&name, None, None);
        let login = st.login_url.clone().unwrap_or_else(|| st.url("/"));
        let fail = |why: String| {
            tracing::warn!("external sign in refused: {why}");
            Redirect::to(&format!("{login}?params={}", encode(&json!({ "error": "external" }).to_string()))).into_response()
        };
        let Some(flow) = flow.filter(|f| f.expires > now_secs()) else { return fail("no flow".into()) };
        if !q.state.as_deref().is_some_and(|s| crate::server::same(s, &flow.state)) {
            return fail("state mismatch".into());
        }
        let Some(code) = q.code.as_deref().filter(|c| !c.is_empty() && c.len() <= 4096) else { return fail("no code".into()) };
        let Some(redirect) = redirect_uri(&accounts, &st, &provider) else { return fail("no public url".into()) };
        let claims = match oidc.exchange_claims(code, &flow, &redirect).await {
            Ok(c) => c,
            Err(e) => return fail(e),
        };
        let Some(key) = claims.get("sub").and_then(Json::as_str).map(str::to_string) else { return fail("no sub".into()) };
        let (link, remember, back) = (flow.return_to.starts_with('L'), flow.return_to[1..].starts_with('R'), flow.return_to[2..].to_string());
        let users = accounts.users();
        if link {
            let Ok(Some(mut user)) = current(&accounts, &headers).await else { return fail("link without a session".into()) };
            let login = crate::accounts::UserLogin { provider: provider.clone(), display_name: key_display(&claims), provider_key: key };
            return match users.add_login(&mut user, login).await {
                Ok(()) => {
                    let _ = accounts.sign_in().refresh_sign_in(&user).await;
                    Redirect::to(&back).into_response()
                }
                Err(e) => fail(e.to_string()),
            };
        }
        match accounts.sign_in().external_login_sign_in(&provider, &key, remember).await {
            Ok(SignInResult::Succeeded) => Redirect::to(&back).into_response(),
            Ok(SignInResult::RequiresTwoFactor) => Redirect::to(&format!("{}?params={}", st.url("/account/login-2fa"), encode(&json!({ "return_to": back, "remember": remember }).to_string()))).into_response(),
            Ok(SignInResult::LockedOut) => Redirect::to(&st.url("/account/lockout")).into_response(),
            Ok(SignInResult::NotAllowed) => fail("not allowed".into()),
            Ok(_) => {
                let pending = External {
                    provider,
                    key,
                    email: claims.get("email").and_then(Json::as_str).map(str::to_string),
                    verified: claims.get("email_verified").and_then(Json::as_bool).unwrap_or(false),
                    name: claims.get("name").and_then(Json::as_str).map(str::to_string),
                    return_to: back,
                    expires: now_secs() + 600,
                };
                let Some(sealer) = accounts.0.sealer.as_ref() else { return fail("no key".into()) };
                scope::set_cookie(PENDING, Some(&sealer.seal(PENDING, &pending)), Some(600));
                Redirect::to(&st.url("/account/external-login")).into_response()
            }
            Err(e) => fail(e.to_string()),
        }
    }

    fn key_display(claims: &serde_json::Map<String, Json>) -> Option<String> {
        claims.get("email").or_else(|| claims.get("preferred_username")).and_then(Json::as_str).map(|s| s.chars().take(256).collect())
    }
}

#[cfg(feature = "oidc")]
pub(crate) use external::{External, PENDING as EXTERNAL_PENDING};
