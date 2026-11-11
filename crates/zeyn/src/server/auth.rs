//! Routes of the built in sign in: OpenID Connect sign in, callback, logout and token refresh
//! (Cargo feature `oidc`), and the TOTP enrollment and verification pages (feature `totp`).
//! The protocol work lives in `crate::auth`; this module ties it to requests, cookies and pages.

use super::{App, AppState};
use crate::ctx::Identity;
use axum::Router;
use axum::http::HeaderMap;
use std::sync::Arc;

#[cfg(any(feature = "oidc", feature = "totp"))]
use {
    super::{Failure, cookie, same, with_cookie},
    crate::auth::{b64url, now_secs, random, safe_return},
    crate::i18n::Text,
    axum::extract::{Query, State},
    axum::http::{HeaderValue, StatusCode, header},
    axum::response::{IntoResponse, Redirect, Response},
    axum::routing::get,
    serde::Deserialize,
};

/// Sign in settings of an app.
#[derive(Default)]
pub(super) struct Extras {
    /// Where a user goes for a second factor (step up).
    pub mfa_url: Option<String>,
    /// Where a user signs in again for `reauth` actions, unless the app set `reauth_url`.
    pub reauth_url: Option<String>,
    /// Where an idle user is signed out, unless the app set `logout_url`.
    pub logout_url: Option<String>,
    /// Path under the app's base that the runtime posts to (with the CSRF token) to end the
    /// session of an idle user, before it goes to `logout_url` or the sign in page.
    pub signout: Option<&'static str>,
    #[cfg(feature = "oidc")]
    oidc: Option<Arc<crate::auth::OidcAuth>>,
    #[cfg(feature = "totp")]
    totp: Option<Arc<crate::auth::Totp>>,
    #[cfg(feature = "accounts")]
    pub accounts: Option<crate::accounts::Accounts>,
}

impl App {
    /// Signs browser users in with an OpenID Connect provider (see [`crate::OidcAuth`]). It is the
    /// app's authenticator, its `login_url` and its idle sign out; actions marked `reauth` and
    /// [`crate::RequireMfa`] guards send users back to the provider.
    #[cfg(feature = "oidc")]
    pub fn oidc(mut self, mut oidc: crate::auth::OidcAuth) -> Self {
        oidc.finish();
        self.setup_problems
            .extend(oidc.problems());
        let oidc = Arc::new(oidc);
        self.auth = Some(oidc.clone());
        self.sign_in.oidc = Some(oidc);
        self
    }

    /// Turns on the built in TOTP second factor (see [`crate::Totp`]) next to the app's
    /// authenticator. [`crate::RequireMfa`] guards send users to its page.
    #[cfg(feature = "totp")]
    pub fn totp(mut self, totp: crate::auth::Totp) -> Self {
        self.setup_problems
            .extend(totp.problems());
        self.sign_in.totp = Some(Arc::new(totp));
        self
    }
}

/// Fills in the sign in addresses once the app's public path is known.
pub(super) fn resolve(extras: &mut Extras, prefix: &str, login_url: &mut Option<String>) {
    #[cfg(feature = "totp")]
    if extras.totp.is_some() {
        extras.mfa_url = Some(format!("{prefix}/_zeyn/auth/totp"));
    }
    #[cfg(feature = "oidc")]
    if extras.oidc.is_some() {
        login_url.get_or_insert_with(|| format!("{prefix}/_zeyn/auth/login"));
        extras.mfa_url = Some(format!("{prefix}/_zeyn/auth/login?mfa=1"));
        extras.reauth_url = Some(format!("{prefix}/_zeyn/auth/login?reauth=1"));
        extras.logout_url = Some(format!("{prefix}/_zeyn/auth/logout"));
    }
    #[cfg(feature = "accounts")]
    if extras.accounts.is_some() {
        let login = login_url
            .get_or_insert_with(|| format!("{prefix}/account/login"))
            .clone();
        extras.mfa_url = Some(login.clone());
        extras.reauth_url = Some(login);
        extras.signout = Some("/_zeyn/account/logout");
    }
    let _ = (extras, prefix, login_url);
}

/// Adds what the request proves beyond the authenticator: a verified TOTP code.
pub(super) fn adjust(st: &AppState, headers: &HeaderMap, identity: &mut Identity) {
    #[cfg(feature = "totp")]
    if let Some(totp) = &st.sign_in.totp {
        totp.apply(cookie(headers, crate::auth::totp::PROOF), identity);
    }
    let _ = (st, headers, identity);
}

/// Layers that must see every request, whatever route it goes to: the OIDC token refresh. Applied
/// once over the whole router, so routes added later are covered too.
pub(super) fn layers(router: Router<Arc<AppState>>, st: &Arc<AppState>) -> Router<Arc<AppState>> {
    #[cfg(feature = "oidc")]
    if st
        .sign_in
        .oidc
        .as_ref()
        .is_some_and(|o| o.refresh)
    {
        return router.layer(axum::middleware::from_fn_with_state(
            st.clone(),
            oidc::refresh,
        ));
    }
    let _ = st;
    router
}

pub(super) fn routes(router: Router<Arc<AppState>>, st: &Arc<AppState>) -> Router<Arc<AppState>> {
    #[cfg(feature = "oidc")]
    let router = if st.sign_in.oidc.is_some() {
        router
            .route("/_zeyn/auth/login", get(oidc::login))
            .route("/_zeyn/auth/callback", get(oidc::callback))
            .route("/_zeyn/auth/logout", get(oidc::logout))
    } else {
        router
    };
    #[cfg(feature = "totp")]
    let router = if st.sign_in.totp.is_some() {
        router
            .route("/_zeyn/auth/totp", get(totp::page))
            .route("/_zeyn/auth/totp/verify", axum::routing::post(totp::verify))
            .route("/_zeyn/auth/totp/enroll", axum::routing::post(totp::enroll))
    } else {
        router
    };
    let _ = st;
    router
}

/// The key the browser runtime files its stored values under (`<meta name="zeyn-storage">`), so
/// the next user of a shared browser never sees the form drafts, grid views or palette recents of
/// the one before. `anon` for signed out users. For a user it is a short keyed hash (HMAC with the
/// app secret of the built in accounts or OpenID Connect sign in), never the id itself. Apps
/// without either hash the id without a key: the value still differs per user and does not show
/// the id at a glance, but whoever can list the possible ids can match them.
pub(super) fn storage_key(st: &AppState, user: Option<&str>) -> String {
    let Some(user) = user else {
        return "anon".into();
    };
    let value = format!("zeyn storage {user}");
    #[cfg(feature = "accounts")]
    if let Some(sealer) = st
        .sign_in
        .accounts
        .as_ref()
        .and_then(|a| a.0.sealer.as_ref())
    {
        return sealer.hash(&value)[..16].to_string();
    }
    #[cfg(feature = "oidc")]
    if let Some(sealer) = st
        .sign_in
        .oidc
        .as_ref()
        .and_then(|o| o.sealer.as_ref())
    {
        return sealer.hash(&value)[..16].to_string();
    }
    let _ = st;
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    value.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// A cookie for this app: HttpOnly, `SameSite=Lax` (it must travel with the provider's redirect
/// back), `Secure` on HTTPS. `None` as value deletes it.
#[cfg(any(feature = "oidc", feature = "totp"))]
fn set_cookie(
    st: &AppState,
    name: &str,
    value: Option<&str>,
    max_age: i64,
    secure: bool,
) -> String {
    let secure = if secure || st.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    match value {
        Some(v) => {
            format!(
                "{name}={v}; Path={}; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
                st.cookie_path(),
                max_age.max(0)
            )
        }
        None => format!(
            "{name}=; Path={}; HttpOnly; SameSite=Lax; Max-Age=0{secure}",
            st.cookie_path()
        ),
    }
}

#[cfg(any(feature = "oidc", feature = "totp"))]
fn add_cookie(mut response: Response, cookie: String) -> Response {
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, v);
    }
    response
}

/// The error page of a sign in that did not work. The reason goes to the log only.
#[cfg(any(feature = "oidc", feature = "totp"))]
fn refused(
    st: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    code: &'static str,
    key: &'static str,
) -> Response {
    let ctx = st.ctx(headers);
    let (csrf, set) = st.csrf(headers);
    with_cookie(
        st.error_page(&ctx, Failure::Status(status, code, Text::key(key)), &csrf),
        set,
    )
}

#[cfg(feature = "oidc")]
mod oidc {
    use super::*;
    use crate::auth::oidc::{FLOW, FLOW_TTL, SESSION};
    use crate::auth::{Flow, OidcAuth};

    #[derive(Deserialize)]
    pub(super) struct LoginQuery {
        return_to: Option<String>,
        mfa: Option<String>,
        reauth: Option<String>,
    }

    fn redirect_uri(st: &AppState, oidc: &OidcAuth) -> String {
        format!("{}{}/_zeyn/auth/callback", oidc.public_url, st.prefix)
    }

    /// Starts a sign in: a new `state`, `nonce` and PKCE verifier in a sealed cookie, and a
    /// redirect to the provider.
    pub(super) async fn login(
        State(st): State<Arc<AppState>>,
        headers: HeaderMap,
        Query(q): Query<LoginQuery>,
    ) -> Response {
        let Some(oidc) = st.sign_in.oidc.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(sealer) = oidc.sealer.as_ref() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let back = q
            .return_to
            .as_deref()
            .and_then(safe_return)
            .map(str::to_string)
            .unwrap_or_else(|| st.url("/"));
        let flow = Flow {
            state: b64url(&random::<32>()),
            nonce: b64url(&random::<32>()),
            verifier: b64url(&random::<32>()),
            return_to: back,
            mfa: q.mfa.is_some(),
            expires: now_secs() + FLOW_TTL,
        };
        match oidc
            .authorize_url(&flow, &redirect_uri(&st, &oidc), q.reauth.is_some())
            .await
        {
            Ok(url) => {
                let value = sealer.seal(FLOW, &flow);
                add_cookie(
                    Redirect::to(&url).into_response(),
                    set_cookie(&st, FLOW, Some(&value), FLOW_TTL, oidc.secure()),
                )
            }
            Err(e) => {
                tracing::error!("sign in is not available: {e}");
                refused(
                    &st,
                    &headers,
                    StatusCode::BAD_GATEWAY,
                    "sign_in_unavailable",
                    "zeyn.auth.unavailable",
                )
            }
        }
    }

    #[derive(Deserialize)]
    pub(super) struct CallbackQuery {
        code: Option<String>,
        state: Option<String>,
        error: Option<String>,
    }

    /// The provider's redirect back: checks `state` against the flow cookie (which is used once),
    /// exchanges the code with the PKCE verifier, checks the ID token and its `nonce`, and starts
    /// the session.
    pub(super) async fn callback(
        State(st): State<Arc<AppState>>,
        headers: HeaderMap,
        Query(q): Query<CallbackQuery>,
    ) -> Response {
        let Some(oidc) = st.sign_in.oidc.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let clear_flow = set_cookie(&st, FLOW, None, 0, oidc.secure());
        let flow = cookie(&headers, FLOW).and_then(|v| {
            oidc.sealer
                .as_ref()?
                .open::<Flow>(FLOW, v)
        });
        let result: Result<(String, Flow, i64), (&'static str, String)> = async {
            let flow = flow.ok_or((
                "zeyn.auth.failed",
                "no sign in in progress: the flow cookie is missing, used or changed".to_string(),
            ))?;
            if flow.expires <= now_secs() {
                return Err(("zeyn.auth.failed", "the sign in took too long".into()));
            }
            if !q
                .state
                .as_deref()
                .is_some_and(|s| same(s, &flow.state))
            {
                return Err(("zeyn.auth.failed", "state mismatch".into()));
            }
            if let Some(e) = &q.error {
                return Err((
                    "zeyn.auth.failed",
                    format!(
                        "the provider answered `{}`",
                        e.chars().take(64).collect::<String>()
                    ),
                ));
            }
            let code = q
                .code
                .as_deref()
                .filter(|c| !c.is_empty() && c.len() <= 4096)
                .ok_or(("zeyn.auth.failed", "no code".to_string()))?;
            let session = oidc
                .exchange(code, &flow, &redirect_uri(&st, &oidc))
                .await
                .map_err(|e| ("zeyn.auth.failed", e))?;
            if flow.mfa && !session.mfa {
                return Err((
                    "zeyn.auth.mfa_missing",
                    "the provider did not report a second factor".into(),
                ));
            }
            let value = oidc
                .seal_session(&session)
                .map_err(|e| ("zeyn.auth.failed", e))?;
            Ok((value, flow, session.expires))
        }
        .await;
        match result {
            Ok((value, flow, expires)) => {
                let r = add_cookie(Redirect::to(&flow.return_to).into_response(), clear_flow);
                add_cookie(
                    r,
                    set_cookie(
                        &st,
                        SESSION,
                        Some(&value),
                        expires - now_secs(),
                        oidc.secure(),
                    ),
                )
            }
            Err((key, why)) => {
                tracing::warn!("sign in refused: {why}");
                add_cookie(
                    refused(
                        &st,
                        &headers,
                        StatusCode::BAD_REQUEST,
                        "sign_in_failed",
                        key,
                    ),
                    clear_flow,
                )
            }
        }
    }

    /// Ends the session here and at the provider, and clears the app's browser storage.
    pub(super) async fn logout(State(st): State<Arc<AppState>>) -> Response {
        let Some(oidc) = st.sign_in.oidc.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let back = format!("{}{}/", oidc.public_url, st.prefix);
        let target = oidc.logout_url(&back).await;
        let mut r = add_cookie(
            Redirect::to(&target).into_response(),
            set_cookie(&st, SESSION, None, 0, oidc.secure()),
        );
        #[cfg(feature = "totp")]
        {
            r = add_cookie(
                r,
                set_cookie(&st, crate::auth::totp::PROOF, None, 0, oidc.secure()),
            );
        }
        r.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        r.headers_mut().insert(
            super::super::CLEAR_SITE_DATA,
            HeaderValue::from_static(r#""storage""#),
        );
        r
    }

    /// Refreshes the tokens of a session whose tokens expired, before the request runs. A refused
    /// refresh (the user was disabled or signed out at the provider) ends the session.
    pub(super) async fn refresh(
        State(st): State<Arc<AppState>>,
        mut req: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> Response {
        let Some(oidc) = st.sign_in.oidc.clone() else {
            return next.run(req).await;
        };
        let due = cookie(req.headers(), SESSION)
            .and_then(|v| oidc.open_session(v))
            .filter(|s| s.check <= now_secs() && s.refresh.is_some());
        let Some(old) = due else {
            return next.run(req).await;
        };
        let renewed = match oidc.renew(&old).await {
            Ok(s) => oidc
                .seal_session(&s)
                .map(|v| (v, s.expires)),
            Err(e) => Err(e),
        };
        let (value, set) = match renewed {
            Ok((v, expires)) => {
                let set = set_cookie(&st, SESSION, Some(&v), expires - now_secs(), oidc.secure());
                (Some(v), set)
            }
            Err(e) => {
                tracing::info!(user = %old.user, "session refresh refused, signing out: {e}");
                (None, set_cookie(&st, SESSION, None, 0, oidc.secure()))
            }
        };
        replace_cookie(req.headers_mut(), SESSION, value.as_deref());
        add_cookie(next.run(req).await, set)
    }

    /// Replaces (or removes) one cookie in the request's `Cookie` header, so the handler sees the
    /// renewed session.
    fn replace_cookie(headers: &mut HeaderMap, name: &str, value: Option<&str>) {
        let mut pairs: Vec<String> = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .map(str::trim)
            .filter(|p| {
                !p.is_empty()
                    && p.split_once('=')
                        .is_none_or(|(k, _)| k != name)
            })
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
}

#[cfg(feature = "totp")]
mod totp {
    use super::*;
    use crate::auth::Totp;
    use crate::auth::totp::{ENROLL, ENROLL_TTL, PROOF, Pending, Proof, base32, qr_svg};
    use crate::ctx::Ctx;
    use crate::render::escape;
    use axum::Form;

    #[derive(Deserialize)]
    pub(super) struct PageQuery {
        return_to: Option<String>,
    }

    #[derive(Deserialize)]
    pub(super) struct CodeForm {
        #[serde(default)]
        csrf: String,
        #[serde(default)]
        code: String,
        return_to: Option<String>,
    }

    /// Associated data of a stored secret: it opens only for the user it was sealed for.
    fn secret_name(user: &str) -> String {
        format!("zeyn_totp_secret:{user}")
    }

    fn back(st: &AppState, target: Option<&str>) -> String {
        target
            .and_then(safe_return)
            .map(str::to_string)
            .unwrap_or_else(|| st.url("/"))
    }

    enum View<'a> {
        Enroll { secret: &'a [u8], error: bool },
        Verify { error: Option<&'static str> },
        Recovery { codes: &'a [String] },
    }

    fn render(
        st: &AppState,
        ctx: &Ctx,
        totp: &Totp,
        csrf: &str,
        user: &str,
        back: &str,
        view: View,
    ) -> String {
        let t = |k: &str| escape(&ctx.t(&Text::key(format!("zeyn.auth.{k}"))));
        let form = |action: &str, error: Option<String>| {
            format!(
                r#"<form class="z-form" method="post" action="{base}/_zeyn/auth/totp/{action}"><input type="hidden" name="csrf" value="{csrf}"><input type="hidden" name="return_to" value="{back}"><div class="z-field"><label for="z-totp-code">{label}</label><input id="z-totp-code" name="code" type="text" inputmode="numeric" autocomplete="one-time-code" required maxlength="16" aria-describedby="z-totp-error"{invalid} autofocus></div><p class="z-field-error" id="z-totp-error" aria-live="polite">{error}</p><div class="z-form-actions"><button type="submit" class="z-button z-primary">{verify}</button></div></form>"#,
                base = escape(&st.prefix),
                csrf = super::super::mask_token(csrf),
                back = escape(back),
                label = t("totp_code"),
                invalid = if error.is_some() {
                    r#" aria-invalid="true""#
                } else {
                    ""
                },
                error = error.unwrap_or_default(),
                verify = t("totp_verify"),
            )
        };
        let body = match view {
            View::Enroll { secret, error } => {
                let url = totp
                    .generator(secret.to_vec(), user)
                    .and_then(|g| g.to_url().ok())
                    .unwrap_or_default();
                let key: Vec<String> = base32(secret)
                    .as_bytes()
                    .chunks(4)
                    .map(|c| String::from_utf8_lossy(c).into_owned())
                    .collect();
                format!(
                    r#"<p>{scan}</p>{qr}<p>{manual} <code class="z-totp-key">{key}</code></p>{form}"#,
                    scan = t("totp_scan"),
                    qr = qr_svg(&url, &ctx.t(&Text::key("zeyn.auth.totp_qr"))).unwrap_or_default(),
                    manual = t("totp_manual"),
                    key = key.join(" "),
                    form = form("enroll", error.then(|| t("totp_invalid"))),
                )
            }
            View::Verify { error } => format!(
                r#"<p>{}</p>{}"#,
                t("totp_intro"),
                form("verify", error.map(t))
            ),
            View::Recovery { codes } => format!(
                r#"<h2>{title}</h2><p>{body}</p><ul class="z-totp-codes">{list}</ul><p><a class="z-button z-primary" href="{back}">{next}</a></p>"#,
                title = t("totp_recovery_title"),
                body = t("totp_recovery_body"),
                list = codes
                    .iter()
                    .map(|c| format!("<li><code>{}</code></li>", escape(c)))
                    .collect::<String>(),
                back = escape(back),
                next = t("totp_continue"),
            ),
        };
        format!(
            r#"<section class="z-screen z-totp" aria-labelledby="z-totp-title"><h1 id="z-totp-title">{}</h1>{body}</section>"#,
            t("totp_title")
        )
    }

    fn html(st: &AppState, ctx: &Ctx, csrf: &str, status: StatusCode, main: String) -> Response {
        let title = ctx.t(&Text::key("zeyn.auth.totp_title"));
        let mut r = (
            status,
            axum::response::Html(st.shell(ctx, csrf, &title, &main, None)),
        )
            .into_response();
        r.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        r
    }

    /// A signed out visitor goes to sign in first, or gets a refusal when the app has no sign in
    /// page.
    fn signed_out(st: &AppState, headers: &HeaderMap, here: &str) -> Response {
        match &st.login_url {
            Some(login) => {
                let sep = if login.contains('?') { '&' } else { '?' };
                Redirect::to(&format!(
                    "{login}{sep}return_to={}",
                    super::super::encode(here)
                ))
                .into_response()
            }
            None => refused(
                st,
                headers,
                StatusCode::FORBIDDEN,
                "forbidden",
                "zeyn.error.forbidden",
            ),
        }
    }

    fn pending_cookie(st: &AppState, totp: &Totp, user: &str, secret: &[u8]) -> Option<String> {
        let p = Pending {
            user: user.to_string(),
            secret: b64url(secret),
            expires: now_secs() + ENROLL_TTL,
        };
        let value = totp.sealer.as_ref()?.seal(ENROLL, &p);
        Some(set_cookie(st, ENROLL, Some(&value), ENROLL_TTL, false))
    }

    fn proof_cookie(st: &AppState, totp: &Totp, user: &str) -> Option<String> {
        let now = now_secs();
        let life = totp.lifetime.as_secs() as i64;
        let value = totp.sealer.as_ref()?.seal(
            PROOF,
            &Proof {
                user: user.to_string(),
                at: now,
                expires: now + life,
            },
        );
        Some(set_cookie(st, PROOF, Some(&value), life, false))
    }

    /// The enrollment page for a user without a second factor, else the verification page.
    pub(super) async fn page(
        State(st): State<Arc<AppState>>,
        headers: HeaderMap,
        Query(q): Query<PageQuery>,
    ) -> Response {
        let Some(totp) = st.sign_in.totp.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let ctx = st.ctx(&headers);
        let (csrf, set) = st.csrf(&headers);
        let back = back(&st, q.return_to.as_deref());
        let Some(user) = ctx.identity().user.clone() else {
            return signed_out(
                &st,
                &headers,
                &format!(
                    "{}/_zeyn/auth/totp?return_to={}",
                    st.prefix,
                    super::super::encode(&back)
                ),
            );
        };
        let response = match totp.store.load(&user).await {
            Err(e) => st.error_page(&ctx, Failure::App(e), &csrf),
            Ok(Some(_)) => html(
                &st,
                &ctx,
                &csrf,
                StatusCode::OK,
                render(
                    &st,
                    &ctx,
                    &totp,
                    &csrf,
                    &user,
                    &back,
                    View::Verify { error: None },
                ),
            ),
            Ok(None) => {
                let secret = random::<20>();
                let main = render(
                    &st,
                    &ctx,
                    &totp,
                    &csrf,
                    &user,
                    &back,
                    View::Enroll {
                        secret: &secret,
                        error: false,
                    },
                );
                let r = html(&st, &ctx, &csrf, StatusCode::OK, main);
                match pending_cookie(&st, &totp, &user, &secret) {
                    Some(c) => add_cookie(r, c),
                    None => r,
                }
            }
        };
        with_cookie(response, set)
    }

    /// Common checks of a posted code: the CSRF token, a signed in user and the attempt limit.
    #[allow(
        clippy::result_large_err,
        reason = "the error is the HTTP response, returned to the client at once and never stored; boxing it would allocate on every refusal"
    )]
    async fn admit(
        st: &AppState,
        headers: &HeaderMap,
        totp: &Totp,
        f: &CodeForm,
    ) -> Result<(Ctx, String), Response> {
        let ctx = st.ctx(headers);
        let token_ok =
            cookie(headers, "zeyn_csrf").is_some_and(|c| super::super::csrf_matches(c, &f.csrf));
        if !token_ok {
            let csrf = st.csrf(headers).0;
            return Err(st.error_page(
                &ctx,
                Failure::App(crate::Error::Denied(Text::key("zeyn.error.csrf"))),
                &csrf,
            ));
        }
        let Some(user) = ctx.identity().user.clone() else {
            return Err(signed_out(
                st,
                headers,
                &format!("{}/_zeyn/auth/totp", st.prefix),
            ));
        };
        if st
            .rate_store
            .take(&format!("totp:{user}"), &totp.attempts)
            .await
            .is_err()
        {
            let main = render(
                st,
                &ctx,
                totp,
                &f.csrf,
                &user,
                &back(st, f.return_to.as_deref()),
                View::Verify {
                    error: Some("totp_rate_limited"),
                },
            );
            return Err(html(st, &ctx, &f.csrf, StatusCode::TOO_MANY_REQUESTS, main));
        }
        Ok((ctx, user))
    }

    /// Checks a code or a recovery code, records its use, and proves the second factor.
    pub(super) async fn verify(
        State(st): State<Arc<AppState>>,
        headers: HeaderMap,
        Form(f): Form<CodeForm>,
    ) -> Response {
        let Some(totp) = st.sign_in.totp.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let (ctx, user) = match admit(&st, &headers, &totp, &f).await {
            Ok(v) => v,
            Err(r) => return r,
        };
        let back = back(&st, f.return_to.as_deref());
        let Some(_turn) = totp.in_flight.enter(&user) else {
            let main = render(
                &st,
                &ctx,
                &totp,
                &f.csrf,
                &user,
                &back,
                View::Verify {
                    error: Some("totp_invalid"),
                },
            );
            return html(&st, &ctx, &f.csrf, StatusCode::BAD_REQUEST, main);
        };
        let mut record = match totp.store.load(&user).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                return Redirect::to(&format!(
                    "{}/_zeyn/auth/totp?return_to={}",
                    st.prefix,
                    super::super::encode(&back)
                ))
                .into_response();
            }
            Err(e) => return st.error_page(&ctx, Failure::App(e), &f.csrf),
        };
        let Some(sealer) = totp.sealer.as_ref() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let secret = sealer
            .open::<String>(&secret_name(&user), &record.secret)
            .and_then(|s| crate::auth::b64url_decode(&s));
        let step = secret
            .and_then(|s| totp.check(s, &user, &f.code))
            .filter(|s| *s > record.last_step);
        let accepted = if let Some(step) = step {
            record.last_step = step;
            true
        } else if let Some(hash) = totp.recovery_hash(&f.code) {
            let before = record.recovery.len();
            record
                .recovery
                .retain(|h| !same(h, &hash));
            record.recovery.len() < before
        } else {
            false
        };
        if !accepted {
            tracing::info!(user = %user, "second factor refused");
            let main = render(
                &st,
                &ctx,
                &totp,
                &f.csrf,
                &user,
                &back,
                View::Verify {
                    error: Some("totp_invalid"),
                },
            );
            return html(&st, &ctx, &f.csrf, StatusCode::BAD_REQUEST, main);
        }
        if let Err(e) = totp.store.save(&user, record).await {
            return st.error_page(&ctx, Failure::App(e), &f.csrf);
        }
        let r = Redirect::to(&back).into_response();
        match proof_cookie(&st, &totp, &user) {
            Some(c) => add_cookie(r, c),
            None => r,
        }
    }

    /// Finishes an enrollment: the code must match the secret from the enrollment cookie. A user
    /// who already has a second factor cannot replace it here.
    pub(super) async fn enroll(
        State(st): State<Arc<AppState>>,
        headers: HeaderMap,
        Form(f): Form<CodeForm>,
    ) -> Response {
        let Some(totp) = st.sign_in.totp.clone() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let (ctx, user) = match admit(&st, &headers, &totp, &f).await {
            Ok(v) => v,
            Err(r) => return r,
        };
        let back = back(&st, f.return_to.as_deref());
        let again = || {
            Redirect::to(&format!(
                "{}/_zeyn/auth/totp?return_to={}",
                st.prefix,
                super::super::encode(&back)
            ))
            .into_response()
        };
        match totp.store.load(&user).await {
            Ok(None) => {}
            Ok(Some(_)) => return again(),
            Err(e) => return st.error_page(&ctx, Failure::App(e), &f.csrf),
        }
        let Some(sealer) = totp.sealer.as_ref() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let pending = cookie(&headers, ENROLL)
            .and_then(|v| sealer.open::<Pending>(ENROLL, v))
            .filter(|p| p.user == user && p.expires > now_secs());
        let Some(secret) = pending.and_then(|p| crate::auth::b64url_decode(&p.secret)) else {
            return again();
        };
        let Some(step) = totp.check(secret.clone(), &user, &f.code) else {
            let main = render(
                &st,
                &ctx,
                &totp,
                &f.csrf,
                &user,
                &back,
                View::Enroll {
                    secret: &secret,
                    error: true,
                },
            );
            return html(&st, &ctx, &f.csrf, StatusCode::BAD_REQUEST, main);
        };
        let Some((codes, hashes)) = totp.recovery_codes() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let record = crate::auth::TotpRecord {
            secret: sealer.seal(&secret_name(&user), &b64url(&secret)),
            recovery: hashes,
            last_step: step,
        };
        if let Err(e) = totp.store.save(&user, record).await {
            return st.error_page(&ctx, Failure::App(e), &f.csrf);
        }
        tracing::info!(user = %user, "second factor enrolled");
        let main = render(
            &st,
            &ctx,
            &totp,
            &f.csrf,
            &user,
            &back,
            View::Recovery { codes: &codes },
        );
        let r = add_cookie(
            html(&st, &ctx, &f.csrf, StatusCode::OK, main),
            set_cookie(&st, ENROLL, None, 0, false),
        );
        match proof_cookie(&st, &totp, &user) {
            Some(c) => add_cookie(r, c),
            None => r,
        }
    }
}
