//! Endpoints for personal data protection: revealing masked values.

use super::{AppState, Call, CallKind, Failure};
use crate::action::{Access, Error};
use crate::audit::FieldRef;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::time::Instant;

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/_zeyn/reveal", post(reveal))
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
