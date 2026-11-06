//! Endpoints of the component library: its assets, lazy node fragments and data sources.
//!
//! Every call goes through the same checks as grids and forms: CSRF, the screen's access, the
//! feature's guards, then the action's permission and the interceptors.

use super::{AppState, Call, CallKind, Failure, asset};
use crate::action::Error;
use crate::form::{Reading, read_moment};
use crate::i18n::Text;
use crate::render::Render;
use crate::time::{DateWords, range_presets};
use crate::ui::{NodePath, input_text, picker_config};
use crate::value::{FieldKind, Value};
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::Value as Json;
use serde_json::json;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/components.js");
const CSS: &str = include_str!("../../assets/components.css");
const CATALOG_JS: &str = include_str!("../../assets/catalog.js");
const CATALOG_CSS: &str = include_str!("../../assets/catalog.css");
const CONTENT_CSS: &str = include_str!("../../assets/content.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 5] = [
    "/_zeyn/components.js",
    "/_zeyn/components.css",
    "/_zeyn/catalog.js",
    "/_zeyn/catalog.css",
    "/_zeyn/content.css",
];

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/_zeyn/components.js",
            get(|| async { asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/components.css",
            get(|| async { asset(CSS, "text/css; charset=utf-8") }),
        )
        .route(
            "/_zeyn/catalog.js",
            get(|| async { asset(CATALOG_JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/catalog.css",
            get(|| async { asset(CATALOG_CSS, "text/css; charset=utf-8") }),
        )
        .route(
            "/_zeyn/content.css",
            get(|| async { asset(CONTENT_CSS, "text/css; charset=utf-8") }),
        )
        .route("/_zeyn/node", post(node_call))
        .route("/_zeyn/nodes", post(nodes_call))
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
async fn parse_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<ParseCall>,
) -> Response {
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
                other => serde_json::to_value(other)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default(),
            };
            let utc = match &value {
                Value::DateTime(d) => Some(d.to_string()),
                _ => None,
            };
            axum::Json(json!({ "ok": true, "text": input_text(&value, &r), "iso": iso, "utc": utc, "zone": ctx.time_zone().name() }))
                .into_response()
        }
        Err(skipped) => {
            let key = if skipped {
                "zeyn.validation.skipped_time"
            } else {
                "zeyn.ui.date.invalid"
            };
            axum::Json(
                json!({ "ok": false, "message": ctx.t(&Text::key(key).arg("label", label)) }),
            )
            .into_response()
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

/// Most nodes one `POST /_zeyn/nodes` renders.
pub(crate) const MAX_REFRESH_NODES: usize = 32;

#[derive(Deserialize)]
struct NodesCall {
    screen: String,
    #[serde(default)]
    params: Json,
    nodes: Vec<NodePath>,
}

/// Opens a screen with the client's params and renders the nodes at `paths`, each with the first
/// page of its grids, in the order asked. The screen's access, the feature's guards and the
/// interceptors run once for all of them, and the read is recorded like opening the screen.
async fn render_nodes(
    st: &AppState,
    ctx: &crate::ctx::Ctx,
    screen: &str,
    params: Json,
    paths: &[NodePath],
) -> Result<Vec<String>, Failure> {
    let Some((&name, _)) = st.screens.get_key_value(screen) else {
        return Err(Failure::App(Error::NotFound));
    };
    let info = Call {
        kind: CallKind::Screen,
        screen: name,
        action: None,
        via_agent: ctx.via_agent(),
    };
    st.intercept(ctx, &info, || async {
        let (_, entry, screen) = st
            .open(ctx, name, params.clone())
            .await?;
        let mut nodes = Vec::with_capacity(paths.len());
        let mut walked = Vec::new();
        for path in paths {
            nodes.push(
                screen
                    .node(path)
                    .ok_or(Failure::App(Error::NotFound))?,
            );
            walked.extend(screen.walk_from(path));
        }
        super::data::note_screen_read(ctx, &entry.meta, &params);
        let loaded = st
            .preload(ctx, walked, entry.meta.name)
            .await;
        let r = super::user_render(entry.meta.name, ctx);
        let r = Render {
            loaded: Some(&loaded),
            ..r
        };
        Ok::<_, Failure>(
            paths
                .iter()
                .zip(nodes)
                .map(|(path, node)| {
                    let mut out = String::new();
                    crate::ui::render_node(&mut out, &r, path.as_str(), node);
                    out
                })
                .collect(),
        )
    })
    .await
}

/// Renders one node of a screen, with the first page of its grids. Used by lazy tabs.
async fn node_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<NodeCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let paths = [call.node];
    match render_nodes(&st, &ctx, &call.screen, call.params, &paths).await {
        Ok(mut html) => Html(html.pop().unwrap_or_default()).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Renders several nodes of a screen in one request, for the nodes that listen to a refreshed
/// or driven topic: `{ "nodes": { "1.0": "<div class=\"z-node\" ...>" } }`. At most
/// [`MAX_REFRESH_NODES`] distinct paths; a path that is not in the screen answers 404.
async fn nodes_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<NodesCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let mut paths: Vec<NodePath> = Vec::with_capacity(call.nodes.len());
    for path in call.nodes {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if paths.is_empty() || paths.len() > MAX_REFRESH_NODES {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    }
    match render_nodes(&st, &ctx, &call.screen, call.params, &paths).await {
        Ok(html) => {
            let nodes: serde_json::Map<String, Json> = paths
                .iter()
                .zip(html)
                .map(|(p, h)| (p.as_str().to_string(), Json::String(h)))
                .collect();
            axum::Json(json!({ "nodes": nodes })).into_response()
        }
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Runs the action behind a component's data source and returns its JSON.
/// A data source of a component in a header slot: a site header (`screen` is
/// `_site.{feature}`, `feature` is `Some`), where the feature's guards are checked, or the app
/// header (`screen` is `_app`, `feature` is `None`). The action's permission is checked in both,
/// as for a screen's components.
async fn site_data(
    st: &AppState,
    ctx: &crate::ctx::Ctx,
    feature: Option<&str>,
    call: &DataCall,
) -> Response {
    let result = async {
        let (name, header) = match feature {
            Some(feature) => {
                let f = st
                    .features
                    .iter()
                    .find(|f| f.name() == feature)
                    .ok_or(Failure::App(Error::NotFound))?;
                f.check_guards(ctx)
                    .map_err(Failure::App)?;
                (f.name(), f.site_shell().map(|s| &s.header))
            }
            None => ("_app", st.header.as_ref()),
        };
        let source = header
            .and_then(|h| h.node(&call.node))
            .and_then(|n| n.source(&call.key))
            .ok_or(Failure::App(Error::NotFound))?;
        let action = source.action();
        if !ctx.identity().can(&action.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::DataGrid,
            screen: name,
            action: Some(action),
            via_agent: ctx.via_agent(),
        };
        st.intercept(ctx, &info, || {
            st.timed(source.fetch(ctx.clone(), call.input.clone()))
        })
        .await
    }
    .await;
    match result {
        Ok(json) => axum::Json(json).into_response(),
        Err(f) => st.error_response(ctx, f),
    }
}

async fn data_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<DataCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    if let Some(feature) = call.screen.strip_prefix("_site.") {
        return site_data(&st, &ctx, Some(feature), &call).await;
    }
    if call.screen == "_app" {
        return site_data(&st, &ctx, None, &call).await;
    }
    let result = async {
        let (_, entry, screen) = st
            .open(&ctx, &call.screen, call.params)
            .await?;
        let source = screen
            .node(&call.node)
            .and_then(|n| n.source(&call.key))
            .ok_or(Failure::App(Error::NotFound))?;
        let action = source.action();
        if !ctx.identity().can(&action.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::DataGrid,
            screen: entry.meta.name,
            action: Some(action),
            via_agent: ctx.via_agent(),
        };
        st.intercept(&ctx, &info, || {
            st.timed(source.fetch(ctx.clone(), call.input))
        })
        .await
    }
    .await;
    match result {
        Ok(json) => axum::Json(json).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}
