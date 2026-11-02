//! Grid endpoints: rows, saved views, row items, inline edits, bulk actions and export.
//!
//! Every endpoint opens the screen with `AppState::open`, finds its grid with `grid_at` (which
//! checks the grid's read access) and runs application code through `AppState::intercept`.

use super::*;

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/grid", post(grid_call))
        .route("/_zeyn/grid/view", post(grid_view_call))
        .route("/_zeyn/grid/run", post(grid_run_call))
        .route("/_zeyn/grid/bulk", post(grid_bulk_call))
        .route("/_zeyn/grid/edit", post(grid_edit_call))
        .route("/_zeyn/grid/export", get(grid_export))
}

/// The grid at `node` of an opened screen, after the grid's read access. Every grid endpoint
/// that addresses a grid starts here, so none can skip the check.
fn grid_at<'s>(
    ctx: &Ctx,
    screen: &'s Screen,
    node: &NodePath,
) -> Result<&'s dyn GridSource, Failure> {
    let grid = screen
        .node(node)
        .and_then(|n| n.grid())
        .ok_or(Failure::App(Error::NotFound))?;
    if !ctx.identity().can(&grid.action().access) {
        return Err(Failure::App(Error::Forbidden));
    }
    Ok(grid)
}

#[derive(Deserialize)]
pub(super) struct GridCall {
    pub(super) screen: String,
    #[serde(default)]
    pub(super) params: Json,
    pub(super) node: NodePath,
    #[serde(default)]
    pub(super) query: DataQuery,
    /// Column layout to render the rows in.
    #[serde(default)]
    pub(super) view: Option<DataGridView>,
    /// Whether to send the header row back too (after the layout changed).
    #[serde(default)]
    pub(super) head: bool,
    /// Group values or ancestor keys of the branch to load in a grouped or tree grid.
    #[serde(default)]
    pub(super) path: Vec<Option<String>>,
}

#[derive(Deserialize)]
struct ExportQuery {
    screen: String,
    params: Option<String>,
    node: crate::ui::NodePath,
    /// JSON of `{ "sort": [...], "filters": [...], "columns": [...] }`, all optional.
    q: Option<String>,
    /// `csv` (the default) or `xlsx`.
    format: Option<String>,
}

#[derive(Deserialize)]
struct EditCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: crate::ui::NodePath,
    key: String,
    /// Typed values of the edited cells and the row version, as a form sends them.
    values: Map<String, Json>,
    /// Layout to render the saved row in.
    #[serde(default)]
    view: Option<DataGridView>,
}

#[derive(Deserialize)]
struct BulkCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: crate::ui::NodePath,
    action: usize,
    /// Keys picked one by one; ignored when `all` is set.
    #[serde(default)]
    keys: Vec<String>,
    /// Every row that matches `query` (its filters); the server collects the keys.
    #[serde(default)]
    all: bool,
    #[serde(default)]
    query: DataQuery,
}

#[derive(Deserialize)]
struct RowItemCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: crate::ui::NodePath,
    /// Key of the row, as in `data-zeyn-key`.
    key: String,
    /// Which item: `a0` is the first row action.
    item: String,
}

#[derive(Deserialize)]
struct DataGridViewCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: NodePath,
    /// The view to save; `null` deletes the saved view.
    view: Option<DataGridView>,
}

async fn grid_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<GridCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.run_grid(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

/// Streams every row that matches the filters as CSV (or XLSX with the `xlsx` feature), page by
/// page through the grid's action, in the user's column layout and language, up to the grid's
/// export limit. With the `pdf` feature, `format=pdf` prints a list report instead. Needs the
/// grid's read access and its export access; interceptors see the call.
async fn grid_export(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ExportQuery>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let xlsx = match q.format.as_deref() {
        None | Some("csv") => false,
        #[cfg(feature = "xlsx")]
        Some("xlsx") => true,
        #[cfg(feature = "pdf")]
        Some("pdf") => false,
        Some(_) => return st.error_response(&ctx, Failure::App(Error::NotFound)),
    };
    let opened = async {
        let params = match &q.params {
            None => Json::Null,
            Some(p) => serde_json::from_str(p).map_err(|_| Failure::App(Error::NotFound))?,
        };
        let spec: Json = match &q.q {
            None => Json::Null,
            Some(s) if s.len() <= 64 * 1024 => {
                serde_json::from_str(s).map_err(|_| Failure::App(Error::NotFound))?
            }
            Some(_) => return Err(Failure::App(Error::NotFound)),
        };
        let (_, entry, screen) = st.open(&ctx, &q.screen, params).await?;
        let grid = screen
            .node(&q.node)
            .and_then(|n| n.grid())
            .ok_or(Failure::App(Error::NotFound))?;
        let access = grid.export_access().ok_or(Failure::App(Error::NotFound))?;
        if !ctx.identity().can(&grid.action().access) || !ctx.identity().can(&access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::DataGrid,
            screen: entry.meta.name,
            action: Some(grid.action()),
            via_agent: ctx.via_agent(),
        };
        let checked = st.before(&ctx, &info);
        if checked.is_err() {
            st.after(&ctx, &info, Instant::now(), &checked);
        }
        checked?;
        Ok((entry, screen, spec, info))
    }
    .await;
    let (entry, screen, spec, info) = match opened {
        Ok(v) => v,
        Err(f) => return st.error_response(&ctx, f),
    };
    let view: Option<DataGridView> = serde_json::from_value(
        json!({ "columns": spec.get("columns").cloned().unwrap_or(json!([])) }),
    )
    .ok();
    let mut query: DataQuery = serde_json::from_value(json!({
        "sort": spec.get("sort").cloned().unwrap_or(json!([])),
        "filters": spec.get("filters").cloned().unwrap_or(json!([])),
    }))
    .unwrap_or_default();
    query.with_total = false;
    #[cfg(feature = "pdf")]
    if q.format.as_deref() == Some("pdf") {
        return pdf::grid_report(
            &st,
            &ctx,
            entry.meta.name,
            &screen,
            &q.node,
            view,
            query,
            &info,
        )
        .await;
    }
    let name = entry.meta.name;
    let node = q.node;
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    let state = st.clone();
    tokio::spawn(async move {
        let started = Instant::now();
        let Some(grid) = screen.node(&node).and_then(|n| n.grid()) else {
            return;
        };
        let view = view.map(|v| grid.clean_view(v));
        let r = Render::new(name, ctx.locale(), ctx.i18n())
            .with_tz(ctx.time_zone())
            .with_identity(ctx.identity());
        #[cfg(feature = "xlsx")]
        let sheet = xlsx.then(|| {
            xlsx::XlsxSink::open(
                grid.export_columns(&r, view.as_ref()),
                ctx.locale().to_string(),
                tx.clone(),
            )
        });
        #[cfg(not(feature = "xlsx"))]
        let sheet: Option<std::convert::Infallible> = None;
        if sheet.is_none() {
            let head = format!("\u{feff}{}", grid.csv_head(&r, view.as_ref()));
            if tx.send(Ok(head.into())).await.is_err() {
                return;
            }
        }
        let limit = grid.export_limit();
        let mut sent = 0usize;
        let mut outcome: Result<(), Failure> = Ok(());
        while sent < limit {
            let size = (limit - sent).min(crate::action::MAX_PAGE_SIZE as usize) as u32;
            let mut q = query.clone();
            q.page = crate::action::PageRequest::Offset {
                offset: sent as u64,
                limit: size,
            };
            let mut page = match state.timed(grid.fetch(ctx.clone(), q)).await {
                Ok(p) => p,
                Err(f) => {
                    outcome = Err(f);
                    let _ = tx.send(Err(std::io::Error::other("export failed"))).await;
                    break;
                }
            };
            page.view = view.clone();
            let n = page.count;
            let open = match &sheet {
                _ if n == 0 => true,
                #[cfg(feature = "xlsx")]
                Some(s) => s.send(grid.export_values(&page, &r)).await,
                #[cfg(not(feature = "xlsx"))]
                Some(never) => match *never {},
                None => tx.send(Ok(grid.csv_rows(&page, &r).into())).await.is_ok(),
            };
            if !open {
                break;
            }
            sent += n;
            if n < size as usize {
                break;
            }
        }
        #[cfg(feature = "xlsx")]
        if let Some(s) = sheet
            && let Err(e) = s.finish(outcome.is_ok()).await
        {
            tracing::warn!("XLSX export did not finish: {e}");
        }
        state.after(&ctx, &info, started, &outcome);
    });
    let mut response = axum::body::Body::from_stream(Chunks(rx)).into_response();
    let h = response.headers_mut();
    let (kind, ext) = if xlsx {
        (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xlsx",
        )
    } else {
        ("text/csv; charset=utf-8", "csv")
    };
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}.{ext}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    response
}

async fn grid_edit_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<EditCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.run_edit(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

async fn grid_bulk_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<BulkCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.run_bulk(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

async fn grid_run_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<RowItemCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.run_row_item(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

async fn grid_view_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<DataGridViewCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.save_view(&ctx, call).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

impl AppState {
    pub(super) async fn run_grid(&self, ctx: &Ctx, call: GridCall) -> Result<Json, Failure> {
        if call.path.len() > 32 || call.path.iter().flatten().any(|k| k.len() > 200) {
            return Err(Failure::App(Error::NotFound));
        }
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let grid = grid_at(ctx, &screen, &call.node)?;
        let action = grid.action();
        let info = Call {
            kind: CallKind::DataGrid,
            screen: entry.meta.name,
            action: Some(action),
            via_agent: ctx.via_agent(),
        };
        let mut query = call.query;
        crate::privacy::strip_masked(&mut query, grid.fields(), ctx);
        self.intercept(ctx, &info, move || async move {
            let mut page = self
                .timed(grid.fetch_at(ctx.clone(), query, call.path))
                .await?;
            page.view = call.view.map(|v| grid.clean_view(v));
            let r = user_render(entry.meta.name, ctx);
            Ok(json!({
                "head": call.head.then(|| grid.head_html(&r, page.view.as_ref())),
                "rows": grid.rows_html(&page, &r),
                "foot": grid.foot_html(&page, &r),
                "count": page.count,
                "total": page.total,
                "next": page.next,
                "offset": page.offset,
            }))
        })
        .await
    }

    /// Saves or deletes the user's view of a grid. Needs a store, a signed in user and the grid's
    /// read access; the view is cleaned by the grid first.
    async fn save_view(&self, ctx: &Ctx, call: DataGridViewCall) -> Result<(), Failure> {
        let store = self.views.as_ref().ok_or(Failure::App(Error::NotFound))?;
        let user = ctx
            .identity()
            .user
            .clone()
            .ok_or(Failure::App(Error::Forbidden))?;
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let grid = grid_at(ctx, &screen, &call.node)?;
        let view = call.view.map(|v| grid.clean_view(v));
        self.timed(store.save(
            &user,
            &Self::view_key(ctx, entry.meta.name, &call.node),
            view,
        ))
        .await
    }

    /// Runs a row item. The row is loaded again by key through the grid's action (with the
    /// grid's read access), the item is built from that row, its action's access is checked and
    /// interceptors run. The client only names the row and the item; it never sends the input.
    async fn run_row_item(&self, ctx: &Ctx, call: RowItemCall) -> Result<Json, Failure> {
        if call.key.len() > 200 || call.item.len() > 32 {
            return Err(Failure::App(Error::NotFound));
        }
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let grid = grid_at(ctx, &screen, &call.node)?;
        let page = self
            .timed(grid.find_row(ctx.clone(), call.key.clone()))
            .await?;
        let row = grid
            .row_call(&page, &call.key, &call.item)
            .ok_or(Failure::App(Error::NotFound))?;
        if !ctx.identity().can(&row.meta.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::DataGrid,
            screen: entry.meta.name,
            action: Some(row.meta),
            via_agent: ctx.via_agent(),
        };
        let result = self
            .intercept(ctx, &info, || self.timed((row.run)(ctx.clone())))
            .await;
        result?;
        Ok(json!({ "effects": [] }))
    }

    /// Saves an inline edit: the row is loaded again by key, the values are applied to the
    /// editable columns only and validated like a form, the save action's access is checked,
    /// interceptors run, and the saved row comes back as HTML.
    async fn run_edit(&self, ctx: &Ctx, call: EditCall) -> Result<Json, Failure> {
        if call.key.len() > 200 || call.values.len() > 64 {
            return Err(Failure::App(Error::NotFound));
        }
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let grid = grid_at(ctx, &screen, &call.node)?;
        let page = self
            .timed(grid.find_row(ctx.clone(), call.key.clone()))
            .await?;
        let save = grid.edit_call(
            &page,
            &call.key,
            &call.values,
            ctx.locale(),
            ctx.time_zone(),
        )?;
        if !ctx.identity().can(&save.meta.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::Form,
            screen: entry.meta.name,
            action: Some(save.meta),
            via_agent: ctx.via_agent(),
        };
        let result = self
            .intercept(ctx, &info, || self.timed((save.run)(ctx.clone())))
            .await;
        result?;
        let mut page = self
            .timed(grid.find_row(ctx.clone(), call.key.clone()))
            .await?;
        page.view = call.view.map(|v| grid.clean_view(v));
        let r = user_render(entry.meta.name, ctx);
        Ok(json!({ "row": grid.row_html(&page, &call.key, &r) }))
    }

    /// Runs a bulk action with the selected keys, or with the keys of every row matching the
    /// filters, which the server collects through the grid's action, page by page.
    async fn run_bulk(&self, ctx: &Ctx, mut call: BulkCall) -> Result<Json, Failure> {
        use crate::grid::MAX_BULK;
        let too_many = || {
            Failure::App(Error::Denied(
                Text::key("zeyn.grid.too_many").arg("n", MAX_BULK.to_string()),
            ))
        };
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let grid = grid_at(ctx, &screen, &call.node)?;
        let meta = grid
            .bulk_meta(call.action)
            .ok_or(Failure::App(Error::NotFound))?;
        if !ctx.identity().can(&meta.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let keys = if call.all {
            let mut keys = Vec::new();
            let mut offset = 0u64;
            loop {
                let mut q = call.query.clone();
                q.page = crate::action::PageRequest::Offset {
                    offset,
                    limit: crate::action::MAX_PAGE_SIZE,
                };
                q.aggregates.clear();
                q.with_total = false;
                let page = self.timed(grid.fetch(ctx.clone(), q)).await?;
                let n = page.count;
                keys.extend(grid.page_keys(&page));
                if keys.len() > MAX_BULK {
                    return Err(too_many());
                }
                if n < crate::action::MAX_PAGE_SIZE as usize {
                    break;
                }
                offset += n as u64;
            }
            keys
        } else {
            if call.keys.len() > MAX_BULK {
                return Err(too_many());
            }
            call.keys.dedup();
            call.keys
                .iter()
                .map(|k| grid.key_json(k))
                .collect::<Option<Vec<_>>>()
                .ok_or(Failure::App(Error::NotFound))?
        };
        if keys.is_empty() {
            return Ok(json!({ "effects": [], "count": 0 }));
        }
        let count = keys.len();
        let run = grid
            .bulk_call(call.action, keys)
            .ok_or(Failure::App(Error::NotFound))?;
        let info = Call {
            kind: CallKind::DataGrid,
            screen: entry.meta.name,
            action: Some(run.meta),
            via_agent: ctx.via_agent(),
        };
        let result = self
            .intercept(ctx, &info, || self.timed((run.run)(ctx.clone())))
            .await;
        result?;
        Ok(json!({ "effects": [], "count": count }))
    }
}
