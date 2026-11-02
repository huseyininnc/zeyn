//! Endpoints and assets of the desktop class shell: the command palette, its commands and dock
//! layouts.
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
        .route("/_zeyn/dock", post(dock_call))
}

#[derive(Deserialize)]
struct DockCall {
    screen: String,
    #[serde(default)]
    params: serde_json::Value,
    node: crate::ui::NodePath,
    /// `false` loads the saved layout; `true` saves `layout` (`null` deletes it).
    #[serde(default)]
    save: bool,
    #[serde(default)]
    layout: Option<crate::ui::DockLayout>,
}

/// Loads or saves the signed in user's layout of a dock, in the app's view store under a key
/// built from the tenant, the screen and the node. `404` without a store (the browser keeps the
/// layout itself) or when the node is not a dock; `403` without a signed in user.
async fn dock_call(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<DockCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let store = st.views.as_ref().ok_or(Failure::App(Error::NotFound))?;
        let user = ctx.identity().user.clone().ok_or(Failure::App(Error::Forbidden))?;
        let (_, entry, screen) = st.open(&ctx, &call.screen, call.params).await?;
        let node = screen.node(&call.node).ok_or(Failure::App(Error::NotFound))?;
        let known = crate::ui::dock_panel_ids(&node.schema()).ok_or(Failure::App(Error::NotFound))?;
        let known: Vec<&str> = known.iter().map(String::as_str).collect();
        let key = format!("{}~dock", AppState::view_key(&ctx, entry.meta.name, &call.node));
        if call.save {
            let view = call.layout.map(|l| l.clean(&known).to_view());
            st.timed(store.save(&user, &key, view)).await?;
            return Ok(None);
        }
        let view = st.timed(store.load(&user, &key)).await?;
        let layout = view.as_ref().and_then(crate::ui::DockLayout::from_view).map(|l| l.clean(&known));
        Ok(Some(json!({ "layout": layout })))
    }
    .await;
    match result {
        Ok(Some(body)) => axum::Json(body).into_response(),
        Ok(None) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
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

