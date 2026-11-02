//! Endpoints of the component library: its assets, lazy node fragments and data sources.
//!
//! Every call goes through the same checks as grids and forms: CSRF, the screen's access, the
//! feature's guards, then the action's permission and the interceptors.

use super::{AppState, Call, CallKind, Failure, asset};
use crate::action::Error;
use crate::render::Render;
use crate::form::{Reading, read_moment};
use crate::i18n::Text;
use crate::time::{DateWords, range_presets};
use crate::ui::{NodePath, input_text, picker_config};
use crate::value::{FieldKind, Value};
use serde_json::json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::Value as Json;
use std::sync::Arc;
use std::time::Instant;

const JS: &str = include_str!("../../assets/components.js");
const CSS: &str = include_str!("../../assets/components.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/components.js", "/_zeyn/components.css"];

/// Links placed in the page head after the runtime's own.
pub(super) fn head(base: &str) -> String {
    format!(
        r#"<link rel="stylesheet" href="{base}/_zeyn/components.css"><script type="module" src="{base}/_zeyn/components.js"></script>"#
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/components.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
        .route("/_zeyn/components.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
        .route("/_zeyn/node", post(node_call))
        .route("/_zeyn/data", post(data_call))
        .route("/_zeyn/parse", post(parse_call))
}

#[derive(Deserialize)]
struct ParseCall {
    /// `date`, `time`, `datetime`, `local` or `presets`.
    kind: String,
    #[serde(default)]
    text: String,
    /// Field label for messages; shown as text only.
    #[serde(default)]
    label: String,
}

/// Reads what was typed into a date or time editor the way the form will, in the user's language,
/// zone and clock, and answers with the canonical text. The browser shows this and never computes
/// values itself. `presets` answers today and the quick ranges in the user's zone.
async fn parse_call(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<ParseCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let words = DateWords::from_i18n(ctx.i18n(), ctx.locale());
    let reading = Reading::new(ctx.locale(), ctx.time_zone(), ctx.now(), &words);
    let r = Render::new("parse", ctx.locale(), ctx.i18n()).with_tz(ctx.time_zone());
    if call.kind == "presets" {
        let today = ctx.today();
        let presets: Vec<Json> = range_presets(today)
            .into_iter()
            .map(|(key, from, to)| {
                json!({ "key": key, "label": ctx.t(&Text::key(format!("zeyn.ui.date.presets.{key}"))), "from": from.to_string(), "to": to.to_string() })
            })
            .collect();
        let mut config = picker_config(&r);
        config["today"] = json!(today.to_string());
        config["presets"] = json!(presets);
        return axum::Json(config).into_response();
    }
    let kind = match call.kind.as_str() {
        "date" => FieldKind::Date,
        "time" => FieldKind::Time,
        "datetime" => FieldKind::DateTime,
        "local" => FieldKind::LocalDateTime,
        _ => return st.error_response(&ctx, Failure::App(Error::NotFound)),
    };
    let label: String = call.label.chars().take(200).collect();
    let text: String = call.text.chars().take(100).collect();
    if text.trim().is_empty() {
        return axum::Json(json!({ "ok": true, "text": "", "iso": "" })).into_response();
    }
    match read_moment(kind, &text, &reading) {
        Ok(value) => {
            let iso = match &value {
                Value::DateTime(d) => d.local(ctx.time_zone()).to_string(),
                other => serde_json::to_value(other).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
            };
            let utc = match &value {
                Value::DateTime(d) => Some(d.to_string()),
                _ => None,
            };
            axum::Json(json!({ "ok": true, "text": input_text(&value, &r), "iso": iso, "utc": utc, "zone": ctx.time_zone().name() }))
                .into_response()
        }
        Err(skipped) => {
            let key = if skipped { "zeyn.validation.skipped_time" } else { "zeyn.ui.date.invalid" };
            axum::Json(json!({ "ok": false, "message": ctx.t(&Text::key(key).arg("label", label)) })).into_response()
        }
    }
}

#[derive(Deserialize)]
struct NodeCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: NodePath,
}

#[derive(Deserialize)]
struct DataCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: NodePath,
    #[serde(default)]
    key: String,
    #[serde(default)]
    input: Json,
}

/// Renders one node of a screen, with the first page of its grids. Used by lazy tabs.
async fn node_call(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<NodeCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let (_, entry, screen) = st.open(&ctx, &call.screen, call.params).await?;
        let node = screen.node(&call.node).ok_or(Failure::App(Error::NotFound))?;
        let loaded = st.preload(&ctx, screen.walk_from(&call.node), entry.meta.name).await;
        let r = Render::new(entry.meta.name, ctx.locale(), ctx.i18n()).with_tz(ctx.time_zone());
        let r = Render { loaded: Some(&loaded), ..r };
        let mut out = String::new();
        crate::ui::render_node(&mut out, &r, call.node.as_str(), node);
        Ok::<_, Failure>(out)
    }
    .await;
    match result {
        Ok(html) => Html(html).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Runs the action behind a component's data source and returns its JSON.
async fn data_call(State(st): State<Arc<AppState>>, headers: HeaderMap, axum::Json(call): axum::Json<DataCall>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let result = async {
        let (_, entry, screen) = st.open(&ctx, &call.screen, call.params).await?;
        let source = screen.node(&call.node).and_then(|n| n.source(&call.key)).ok_or(Failure::App(Error::NotFound))?;
        let action = source.action();
        if !ctx.identity().can(&action.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call { kind: CallKind::Grid, screen: entry.meta.name, action: Some(action), via_agent: ctx.via_agent() };
        let started = Instant::now();
        let result = async {
            st.before(&ctx, &info)?;
            st.timed(source.fetch(ctx.clone(), call.input)).await
        }
        .await;
        st.after(&ctx, &info, started, &result);
        result
    }
    .await;
    match result {
        Ok(json) => axum::Json(json).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}
