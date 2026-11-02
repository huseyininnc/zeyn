//! PDF downloads: documents of [`crate::PdfLink`] components and grid list reports
//! (`Export::Pdf`). Rendering waits for a slot of the [`crate::Pdf`] service and runs on a
//! blocking thread, so a request never blocks the async workers.

use super::{AppState, Call, CallKind, Failure};
use crate::action::{DataQuery, Error};
use crate::ctx::Ctx;
use crate::grid::DataGridView;
use crate::pdf::{MAX_PDF_ROWS, Pdf, PdfTemplate, lang};
use crate::render::Render;
use crate::screen::{GridSource, Screen};
use crate::ui::NodePath;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::time::Instant;

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/_zeyn/pdf", get(document))
}

#[derive(Deserialize)]
struct DocQuery {
    screen: String,
    params: Option<String>,
    node: NodePath,
}

/// A PDF file response that is never cached.
pub(super) fn pdf_response(name: &str, bytes: Vec<u8>) -> Response {
    let mut response = bytes.into_response();
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}.pdf\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    response
}

/// Renders the document of a `PdfLink` on a screen: its action loads the data for the user,
/// with the action's access and interceptors, then the template is rendered.
async fn document(State(st): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<DocQuery>) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let result = async {
        let params = match &q.params {
            None => Json::Null,
            Some(p) => serde_json::from_str(p).map_err(|_| Failure::App(Error::NotFound))?,
        };
        let (_, entry, screen) = st.open(&ctx, &q.screen, params).await?;
        let source = screen.node(&q.node).and_then(|n| n.pdf()).ok_or(Failure::App(Error::NotFound))?;
        let action = source.action();
        if !ctx.identity().can(&action.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call { kind: CallKind::Document, screen: entry.meta.name, action: Some(action), via_agent: ctx.via_agent() };
        let (state, user) = (&*st, &ctx);
        st.intercept(&ctx, &info, move || async move {
            let (template, data) = state.timed(source.load(user.clone())).await?;
            let bytes = state.timed(Pdf::of(user).render(template, &data)).await?;
            Ok((template, bytes))
        })
        .await
    }
    .await;
    match result {
        Ok((template, bytes)) => pdf_response(template.name, bytes),
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Pages through a grid's action and prints the rows as a list report, in the user's layout and
/// language, up to [`MAX_PDF_ROWS`] (or the grid's export limit when smaller).
#[allow(clippy::too_many_arguments)]
pub(super) async fn grid_report(
    st: &AppState,
    ctx: &Ctx,
    name: &'static str,
    screen: &Screen,
    node: &NodePath,
    view: Option<DataGridView>,
    query: DataQuery,
    info: &Call,
) -> Response {
    let started = Instant::now();
    let outcome = async {
        let grid: &dyn GridSource = screen.node(node).and_then(|n| n.grid()).ok_or(Failure::App(Error::NotFound))?;
        let view = view.map(|v| grid.clean_view(v));
        let r = Render::new(name, ctx.locale(), ctx.i18n()).with_tz(ctx.time_zone()).with_identity(ctx.identity());
        let limit = grid.export_limit().min(MAX_PDF_ROWS);
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut more = false;
        while rows.len() < limit {
            let size = (limit - rows.len()).min(crate::action::MAX_PAGE_SIZE as usize) as u32;
            let mut q = query.clone();
            q.page = crate::action::PageRequest::Offset { offset: rows.len() as u64, limit: size };
            let mut page = st.timed(grid.fetch(ctx.clone(), q)).await?;
            page.view = view.clone();
            let n = page.count;
            rows.extend(grid.export_text(&page, &r));
            if n < size as usize {
                break;
            }
            more = rows.len() >= limit;
        }
        let columns: Vec<Json> = grid.export_columns(&r, view.as_ref()).into_iter().map(|c| json!({ "label": c.label, "numeric": c.numeric })).collect();
        let note = more.then(|| ctx.t(&crate::i18n::Text::key("zeyn.export.pdf_cut").arg("max", limit.to_string())));
        let data = json!({
            "title": ctx.t(screen.title()),
            "lang": lang(ctx.locale()),
            "columns": columns,
            "rows": rows,
            "note": note,
        });
        st.timed(Pdf::of(ctx).render(PdfTemplate::LIST, &data)).await
    }
    .await;
    st.after(ctx, info, started, &outcome);
    match outcome {
        Ok(bytes) => pdf_response(name, bytes),
        Err(f) => st.error_response(ctx, f),
    }
}
