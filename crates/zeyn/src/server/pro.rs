//! Endpoints and assets of the desktop class shell: the command palette and its commands.
//!
//! Commands run with the same checks as menu items: CSRF, the feature's guards, the action's
//! permission, then the interceptors.

use super::{AppState, Call, CallKind, Failure, asset};
use crate::action::Error;
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::Render;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

const JS: &str = include_str!("../../assets/pro.js");
const CSS: &str = include_str!("../../assets/pro.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/pro.js", "/_zeyn/pro.css"];

/// Links placed in the page head after the component library's own.
pub(super) fn head(base: &str) -> String {
    format!(r#"<link rel="stylesheet" href="{base}/_zeyn/pro.css"><script type="module" src="{base}/_zeyn/pro.js"></script>"#)
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/pro.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/pro.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
        .route("/_zeyn/command", post(command_call))
}

/// The palette button for the menu and the hidden buttons of every command the user may run,
/// with the palette's texts. Commands of features whose guards deny the user are left out.
pub(super) fn shell_parts(st: &AppState, ctx: &Ctx, r: &Render) -> (String, String) {
    let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.palette.{key}")));
    let button = format!(
        r#"<button type="button" class="z-palette-open" data-zeyn-palette-open aria-haspopup="dialog" aria-keyshortcuts="Control+Shift+P F1"><span class="z-icon z-icon-search" aria-hidden="true"></span><span>{}</span><kbd aria-hidden="true">Ctrl+Shift+P</kbd></button>"#,
        t("open")
    );
    let mut commands = format!(
        r#"<div class="z-commands" hidden data-zeyn-commands data-text-label="{}" data-text-placeholder="{}" data-text-empty="{}" data-text-count="{}" data-text-recent="{}" data-text-app="{}" data-text-screen="{}" data-text-go="{}" data-text-done="{}" data-text-hint="{}">"#,
        t("label"),
        t("placeholder"),
        t("empty"),
        t("count"),
        t("recent"),
        t("app"),
        t("screen"),
        t("go"),
        t("done"),
        t("hint"),
    );
    for f in &st.features {
        if f.check_guards(ctx).is_err() {
            continue;
        }
        for c in f.commands() {
            if ctx.identity().can(&c.access()) {
                commands.push_str(&c.button(r));
            }
        }
    }
    commands.push_str("</div>");
    (button, commands)
}

#[derive(Deserialize)]
struct CommandCall {
    command: String,
}

/// Runs an action command by id. The input was fixed on the server when the command was built;
/// the client only names it. Effect commands run in the browser and are not found here.
async fn command_call(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<CommandCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let (feature, command) = st
            .features
            .iter()
            .find_map(|f| f.commands().iter().find(|c| c.id() == call.command).map(|c| (f, c)))
            .ok_or(Failure::App(Error::NotFound))?;
        feature.check_guards(&ctx)?;
        if !ctx.identity().can(&command.access()) {
            return Err(Failure::App(Error::Forbidden));
        }
        let run = command.call().ok_or(Failure::App(Error::NotFound))?;
        let info = Call { kind: CallKind::Screen, screen: command.id(), action: Some(run.meta), via_agent: ctx.via_agent() };
        let started = Instant::now();
        let result = async {
            st.before(&ctx, &info)?;
            st.timed((run.run)(ctx.clone())).await
        }
        .await;
        st.after(&ctx, &info, started, &result);
        result
    }
    .await;
    match result {
        Ok(()) => axum::Json(json!({ "effects": [] })).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

