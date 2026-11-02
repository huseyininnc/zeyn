//! Endpoints for personal data protection: revealing masked values and reasons for opening
//! screens with personal data.

use super::{AppState, Call, CallKind, Failure, cookie, encode, percent_decode, with_cookie};
use crate::action::{Access, Error};
use crate::audit::{FieldRef, RecordRef};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::model::{FieldError, Rule, ValidationErrors};
use crate::render::escape;
use crate::screen::ScreenMeta;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::post;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::time::Instant;

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/_zeyn/reveal", post(reveal)).route("/_zeyn/reason", post(give_reason))
}

const REASON_COOKIE: &str = "zeyn_reason";
const REASON_SECONDS: u32 = 900;

/// A reason is 3 to 300 characters of text, without control characters.
fn valid_reason(reason: &str) -> bool {
    let n = reason.chars().count();
    (3..=300).contains(&n) && !reason.chars().any(char::is_control)
}

/// Reads the reason cookie (`<screen>.<reason>`) into the request's trail. The server only uses it
/// for that screen.
pub(super) fn read_reason(ctx: &Ctx, headers: &HeaderMap) {
    let Some((screen, reason)) = cookie(headers, REASON_COOKIE).and_then(|v| v.split_once('.')) else { return };
    let reason = percent_decode(reason).trim().to_string();
    if valid_reason(&reason) {
        let screen = screen.to_string();
        ctx.update_trail(|t| t.given_reason = Some((screen, reason)));
    }
}

/// Refuses screens that ask for a reason until the user has given one; records it otherwise.
pub(super) fn check_reason(ctx: &Ctx, meta: &ScreenMeta) -> Result<(), Failure> {
    if !meta.reason {
        return Ok(());
    }
    match ctx.trail().given_reason {
        Some((screen, reason)) if screen == meta.name => {
            ctx.update_trail(|t| t.reason = Some(reason));
            Ok(())
        }
        _ => Err(Failure::Status(StatusCode::PRECONDITION_REQUIRED, "reason_required", Text::key("zeyn.privacy.reason_required"))),
    }
}

/// Records the opening of a screen marked `sensitive` as a read, with its params.
pub(super) fn note_screen_read(ctx: &Ctx, meta: &ScreenMeta, params: &Json) {
    if meta.sensitive || meta.reason {
        let id = (!params.is_null()).then(|| params.to_string());
        ctx.note_read(RecordRef { model: meta.name, id });
    }
}

/// Page that asks for a reason before a screen's data is shown.
pub(super) fn reason_page(st: &AppState, ctx: &Ctx, screen: &str, csrf: &str) -> Response {
    let text = |k: &str| escape(&ctx.t(&Text::key(format!("zeyn.privacy.{k}"))));
    let body = format!(
        r#"<section class="z-screen z-reason" aria-labelledby="z-reason-title"><h1 id="z-reason-title">{title}</h1><p id="z-reason-help">{explain}</p><form class="z-form" data-zeyn-reason="{screen}" novalidate><div class="z-field"><label for="z-reason-text">{label}<span class="z-required" aria-hidden="true">*</span></label><textarea id="z-reason-text" name="reason" required aria-required="true" minlength="3" maxlength="300" rows="3" aria-describedby="z-reason-help z-reason-error"></textarea><p class="z-field-error" id="z-reason-error" aria-live="polite"></p></div><div class="z-form-actions"><button type="submit" class="z-button z-primary">{go}</button></div></form></section>"#,
        title = text("reason_title"),
        explain = text("reason_explain"),
        label = text("reason_label"),
        go = text("reason_continue"),
        screen = escape(screen),
    );
    let mut r = Html(st.shell(ctx, csrf, &ctx.t(&Text::key("zeyn.privacy.reason_title")), &body, Some(screen))).into_response();
    *r.status_mut() = StatusCode::PRECONDITION_REQUIRED;
    r
}

#[derive(Deserialize)]
struct ReasonCall {
    screen: String,
    reason: String,
}

/// Stores the user's reason for opening a screen for 15 minutes, in an `HttpOnly` cookie. The
/// reason is written to the audit record of every call to that screen.
async fn give_reason(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<ReasonCall>) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let Some((name, (_, entry))) = st.screens.get_key_value(call.screen.as_str()) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    if !entry.meta.reason {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    }
    if !ctx.identity().can(&entry.meta.access) {
        return st.error_response(&ctx, Failure::App(Error::Forbidden));
    }
    let reason = call.reason.trim();
    if !valid_reason(reason) {
        let mut errors = ValidationErrors::default();
        errors.errors.push(FieldError { field: "reason", rule: Rule::Custom, message: Text::key("zeyn.privacy.reason_invalid") });
        return st.error_response(&ctx, Failure::App(Error::Validation(errors)));
    }
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    let c = format!(
        "{REASON_COOKIE}={name}.{}; Path={}; Max-Age={REASON_SECONDS}; HttpOnly; SameSite=Strict{secure}",
        encode(reason),
        st.cookie_path()
    );
    with_cookie(StatusCode::NO_CONTENT.into_response(), Some(c))
}

#[derive(Deserialize)]
struct RevealCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: usize,
    field: String,
    /// Row key, for grids.
    key: Option<String>,
}

/// Sends the raw value of one masked field to a user with its unmask permission. Every attempt is
/// audited, including refused ones.
async fn reveal(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<RevealCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let screen_name: &'static str = match st.screens.get_key_value(call.screen.as_str()) {
        Some((name, _)) => *name,
        None => return st.error_response(&ctx, Failure::App(Error::NotFound)),
    };
    let info = Call { kind: CallKind::Reveal, screen: screen_name, action: None, via_agent: false };
    let started = Instant::now();
    let result = async {
        st.before(&ctx, &info)?;
        let (_, _, screen) = st.open(&ctx, screen_name, call.params).await?;
        let node = screen.nodes().get(call.node).ok_or(Failure::App(Error::NotFound))?;
        let found = match node.grid() {
            Some(grid) => {
                if !ctx.identity().can(&grid.action().access) {
                    return Err(Failure::App(Error::Forbidden));
                }
                let key = call.key.clone().ok_or(Failure::App(Error::NotFound))?;
                st.timed(grid.reveal(ctx.clone(), key, call.field.clone())).await?
            }
            None => node.reveal(&call.field),
        };
        let found = found.ok_or(Failure::App(Error::NotFound))?;
        let permission = found.meta.mask.map(|m| m.permission).ok_or(Failure::App(Error::NotFound))?;
        if !ctx.identity().can(&Access::Permission(permission)) {
            return Err(Failure::App(Error::Forbidden));
        }
        ctx.update_trail(|t| {
            t.revealed.push(FieldRef { model: found.record.model, id: found.record.id.clone(), field: found.meta.name })
        });
        Ok(found.value)
    }
    .await;
    st.after(&ctx, &info, started, &result);
    match result {
        Ok(value) => {
            let mut r = axum::Json(json!({ "value": value })).into_response();
            r.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        Err(f) => st.error_response(&ctx, f),
    }
}
