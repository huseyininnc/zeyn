//! Session safety: idle timeout warnings and recent sign in for sensitive actions.
//!
//! Sign in is done by a single sign on proxy, by the built in OpenID Connect sign in or by the app's
//! own authenticator. This module tells the runtime when to warn and where to send the user, and it
//! refuses actions marked `reauth` when the identity provider reports an old sign in. With
//! `OidcAuth` a fresh sign in goes back to the provider with `prompt=login`; with `Totp` a
//! verified code counts as a fresh sign in.

use super::{App, AppState, Call, Failure};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::attr;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// Session settings of an app.
pub(super) struct Settings {
    idle: Option<Duration>,
    warning: Duration,
    logout_url: Option<String>,
    pub(super) reauth_within: Duration,
    reauth_url: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            idle: None,
            warning: Duration::from_secs(60),
            logout_url: None,
            reauth_within: Duration::from_secs(300),
            reauth_url: None,
        }
    }
}

impl App {
    /// Signs users out of the page after this long without input. The runtime warns `warning`
    /// before (one minute unless set with `idle_warning`) in a dialog with a countdown and a
    /// "stay signed in" button, then sends the user to `logout_url`, else to `login_url`, else
    /// shows a signed out screen. Activity in any tab of the app counts.
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.extras.session.idle = Some(timeout);
        self
    }

    /// How long before the idle timeout the warning appears.
    pub fn idle_warning(mut self, before: Duration) -> Self {
        self.extras.session.warning = before;
        self
    }

    /// Where an idle user is sent, such as the proxy's sign out address, so the single sign on
    /// session ends too.
    pub fn logout_url(mut self, url: impl Into<String>) -> Self {
        self.extras.session.logout_url = Some(url.into());
        self
    }

    /// How recent a sign in actions marked `reauth` need. Five minutes unless set.
    pub fn reauth_within(mut self, age: Duration) -> Self {
        self.extras.session.reauth_within = age;
        self
    }

    /// Page where users sign in again for actions marked `reauth`; `login_url` unless set. It is
    /// opened in a new tab with `return_to`, so the work on the page is kept.
    pub fn reauth_url(mut self, url: impl Into<String>) -> Self {
        self.extras.session.reauth_url = Some(url.into());
        self
    }
}

/// Refuses actions marked `reauth` unless the user signed in within `reauth_within`. Identities
/// without a sign in time never pass.
pub(super) fn check_reauth(st: &AppState, ctx: &Ctx, call: &Call) -> Result<(), Failure> {
    if !call.action.is_some_and(|a| a.reauth) {
        return Ok(());
    }
    let _ = st;
    recent(ctx)
}

/// Refuses unless the user signed in within `reauth_within` (see [`Ctx::signed_in_recently`]).
pub(super) fn recent(ctx: &Ctx) -> Result<(), Failure> {
    if ctx.signed_in_recently() {
        Ok(())
    } else {
        Err(Failure::Status(
            StatusCode::UNAUTHORIZED,
            "reauth_required",
            Text::key("zeyn.session.reauth_required"),
        ))
    }
}

/// Session settings for the runtime, as a `<meta>` tag. Empty when nothing is configured.
pub(super) fn head(st: &AppState, ctx: &Ctx) -> String {
    let s = &st.extras.session;
    if !ctx.identity().is_signed_in() {
        return String::new();
    }
    let auth = &st.sign_in;
    let config = json!({
        "idle": s.idle.map(|d| d.as_secs()),
        "warn": s.warning.as_secs(),
        "logout": s.logout_url.as_ref().or(auth.logout_url.as_ref()),
        "login": st.login_url,
        "reauth": s.reauth_url.as_ref().or(auth.reauth_url.as_ref()).or(st.login_url.as_ref()).or(auth.mfa_url.as_ref()),
        "mfa": auth.mfa_url,
    });
    format!(r#"<meta name="zeyn-session" content="{}">"#, attr(&config))
}

/// Keeps the session alive on "stay signed in": a signed in request that a proxy in front sees.
async fn keepalive(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    if !ctx.identity().is_signed_in() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/_zeyn/keepalive", post(keepalive))
}
