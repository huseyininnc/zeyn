use crate::action::{Action, ActionMeta, Cursor, DEFAULT_PAGE_SIZE, GridQuery, MAX_PAGE_SIZE, Page, PageRequest, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::model::{Field, FieldMeta, Model};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Effect, GridSource, LoadedPage, Node};
use crate::value::{FieldType, SupportsSum, Value};
use serde::Serialize;
use serde_json::json;
use std::cmp::Ordering;
use std::marker::PhantomData;

/// Where sorting and filtering happen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GridMode {
    /// Data comes to the client once; sorting and filtering run in the browser.
    #[default]
    Client,
    /// Every page, sort and filter is requested from the server. For large data.
    Server,
}

impl GridMode {
    fn code(self) -> &'static str {
        match self {
            GridMode::Client => "client",
            GridMode::Server => "server",
        }
    }
}

/// How a grid loads rows beyond the first batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Paging {
    /// Numbered pages with a pager. The default.
    Pages { size: u32 },
    /// Loads the next batch when the user scrolls near the end, using a cursor so rows are not
    /// repeated or skipped while data changes. A "Load more" button stays for keyboard and screen
    /// reader users.
    Infinite { batch: u32 },
    /// All rows at once, still capped at [`MAX_PAGE_SIZE`] by the server. For short lists.
    None,
    /// One scrollable list over all rows, however many. Only the visible rows are in the page;
    /// blocks of `block` rows are fetched by offset as the user scrolls and kept in a cache.
    /// Rows have a fixed height so any position can be reached at once.
    Virtual { row_height: u32, block: u32 },
}

impl Default for Paging {
    fn default() -> Self {
        Paging::Pages { size: DEFAULT_PAGE_SIZE }
    }
}

/// Keeps a size between 1 and [`MAX_PAGE_SIZE`].
const fn bounded(size: u32) -> u32 {
    if size == 0 {
        1
    } else if size > MAX_PAGE_SIZE {
        MAX_PAGE_SIZE
    } else {
        size
    }
}

impl Paging {
    /// Numbered pages of `size` rows.
    pub const fn pages(size: u32) -> Self {
        Paging::Pages { size: bounded(size) }
    }

    /// Infinite scroll loading `batch` rows at a time.
    pub const fn infinite(batch: u32) -> Self {
        Paging::Infinite { batch: bounded(batch) }
    }

    /// No paging.
    pub const fn none() -> Self {
        Paging::None
    }

    /// Virtual scrolling with 32 px rows, fetched in blocks of 200. For very large lists.
    pub const fn virtual_scroll() -> Self {
        Paging::Virtual { row_height: 32, block: 200 }
    }

    /// Virtual scrolling with a custom row height in pixels.
    pub const fn virtual_rows(row_height: u32) -> Self {
        Paging::Virtual { row_height: if row_height < 20 { 20 } else { row_height }, block: 200 }
    }

    fn code(self) -> &'static str {
        match self {
            Paging::Pages { .. } => "pages",
            Paging::Infinite { .. } => "infinite",
            Paging::None => "none",
            Paging::Virtual { .. } => "virtual",
        }
    }

    fn size(self) -> u32 {
        match self {
            Paging::Pages { size } => size,
            Paging::Infinite { batch } => batch,
            Paging::None => MAX_PAGE_SIZE,
            Paging::Virtual { block, .. } => bounded(block),
        }
    }
}

/// Row highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowStyle {
    Warning,
    Danger,
    Success,
    Muted,
}

impl RowStyle {
    fn css(self) -> &'static str {
        match self {
            RowStyle::Warning => "z-row-warning",
            RowStyle::Danger => "z-row-danger",
            RowStyle::Success => "z-row-success",
            RowStyle::Muted => "z-row-muted",
        }
    }
}

type ValueFn<E> = Box<dyn Fn(&E) -> Value + Send + Sync>;
type StyleFn<E> = Box<dyn Fn(&E) -> Option<RowStyle> + Send + Sync>;
type OpenFn<E> = Box<dyn Fn(&E) -> Effect + Send + Sync>;
type FetchFn<E> = fn(Ctx, GridQuery) -> BoxFuture<Result<Page<E>>>;

struct Column<E> {
    meta: &'static FieldMeta,
    value: ValueFn<E>,
    width: Option<u32>,
    flex: Option<u32>,
    filter: bool,
    sum: bool,
}

/// Data grid. Its data source is an action that takes a `GridQuery` and returns `Page<E>`.
///
/// Example:
/// ```ignore
/// Grid::of(search_products).column(Product::code).width(120).filter()
/// ```
pub struct Grid<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    mode: GridMode,
    paging: Paging,
    columns: Vec<Column<E>>,
    row_style: Option<StyleFn<E>>,
    on_open: Option<OpenFn<E>>,
}

fn fetch_via<A, E>(ctx: Ctx, query: GridQuery) -> BoxFuture<Result<Page<E>>>
where
    A: Action<Input = GridQuery, Output = Result<Page<E>>>,
    E: Model,
{
    Box::pin(async move { A::call(ctx, query).await? })
}

impl<E: Model> Grid<E> {
    /// Grid that loads its rows from the given action.
    pub fn of<A>(_source: A) -> Self
    where
        A: Action<Input = GridQuery, Output = Result<Page<E>>>,
    {
        Self {
            action: A::META,
            fetch: fetch_via::<A, E>,
            mode: GridMode::default(),
            paging: Paging::default(),
            columns: Vec::new(),
            row_style: None,
            on_open: None,
        }
    }

    /// Grid with a column for every field of the model except the row version, in declaration
    /// order, and filter boxes on text and choice columns. The quickest list for a simple model.
    pub fn auto<A>(source: A) -> Self
    where
        A: Action<Input = GridQuery, Output = Result<Page<E>>>,
    {
        let mut grid = Self::of(source);
        for (i, meta) in E::fields().iter().enumerate().filter(|(_, m)| !m.version) {
            let filter = matches!(meta.kind, crate::value::FieldKind::Text | crate::value::FieldKind::Choice(_));
            grid.columns.push(Column {
                meta,
                value: Box::new(move |e: &E| e.value_at(i).unwrap_or(Value::Null)),
                width: None,
                flex: None,
                filter,
                sum: false,
            });
        }
        grid
    }

    pub fn mode(mut self, mode: GridMode) -> Self {
        self.mode = mode;
        self
    }

    /// Paging style. Numbered pages of 50 rows unless set.
    pub fn paging(mut self, paging: Paging) -> Self {
        self.paging = paging;
        self
    }

    /// Adds a column. The following settings (`width`, `filter`...) apply to it.
    pub fn column<T: FieldType + 'static>(mut self, field: Field<E, T>) -> GridColumn<E, T> {
        self.columns.push(Column {
            meta: field.meta(),
            value: Box::new(move |e| field.get(e).to_value()),
            width: None,
            flex: None,
            filter: false,
            sum: false,
        });
        GridColumn { grid: self, _type: PhantomData }
    }

    /// Picks a highlight per row.
    pub fn row_style(mut self, style: impl Fn(&E) -> Option<RowStyle> + Send + Sync + 'static) -> Self {
        self.row_style = Some(Box::new(style));
        self
    }

    /// What happens when a row is opened (double click or Enter).
    pub fn on_open(mut self, open: impl Fn(&E) -> Effect + Send + Sync + 'static) -> Self {
        self.on_open = Some(Box::new(open));
        self
    }

    /// Effect produced when the row is opened.
    pub fn open_effect(&self, row: &E) -> Option<Effect> {
        self.on_open.as_ref().map(|f| f(row))
    }

    /// Renders rows as HTML in English. For tests and tools; the server uses the user's language.
    pub fn rows_html(&self, rows: &[E]) -> String {
        self.rows(rows, &Render::english("grid"))
    }

    fn rows(&self, rows: &[E], r: &Render) -> String {
        let mut out = String::new();
        for row in rows {
            out.push_str("<tr");
            if let Some(style) = self.row_style.as_ref().and_then(|f| f(row)) {
                out.push_str(&format!(r#" class="{}""#, style.css()));
            }
            if let Some(effect) = self.open_effect(row) {
                out.push_str(&format!(r#" tabindex="-1" data-zeyn-effect="{}""#, attr(&effect)));
            }
            out.push('>');
            for c in &self.columns {
                let value = (c.value)(row);
                let class = if matches!(value, Value::Int(_) | Value::Money(_)) { r#" class="z-num""# } else { "" };
                out.push_str(&format!("<td{class}>{}</td>", r.value(&value, &c.meta.kind)));
            }
            out.push_str("</tr>");
        }
        out
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let scope = r.scope;
        let grid_text = |key: &str| r.text(&Text::key(format!("zeyn.grid.{key}")));
        out.push_str(&format!(
            r#"<div class="z-grid" data-zeyn-grid data-testid="{scope}.grid" data-zeyn-action="{}" data-zeyn-mode="{}" data-zeyn-paging="{}" data-zeyn-page-size="{}" data-zeyn-openable="{}" data-zeyn-text-range="{}" data-zeyn-text-range-total="{}" data-zeyn-text-loaded="{}""#,
            self.action.name,
            self.mode.code(),
            self.paging.code(),
            self.paging.size(),
            self.on_open.is_some(),
            grid_text("range"),
            grid_text("range_total"),
            grid_text("loaded"),
        ));
        if let Paging::Virtual { row_height, .. } = self.paging {
            out.push_str(&format!(r#" data-zeyn-row-height="{row_height}" style="--z-row-height:{row_height}px""#));
        }
        if let Some(p) = page {
            out.push_str(&format!(r#" data-zeyn-offset="{}" data-zeyn-count="{}""#, p.offset, p.count));
            if let Some(t) = p.total {
                out.push_str(&format!(r#" data-zeyn-total="{t}""#));
            }
            if let Some(n) = &p.next {
                out.push_str(&format!(r#" data-zeyn-next="{}""#, escape(&n.0)));
            }
        }
        out.push_str(r#"><table role="grid"><thead><tr>"#);
        for c in &self.columns {
            let mut style = String::new();
            if let Some(w) = c.width {
                style.push_str(&format!("width:{w}px;"));
            }
            if let Some(f) = c.flex {
                style.push_str(&format!("--z-flex:{f};"));
            }
            let num = matches!(c.meta.kind, crate::value::FieldKind::Integer | crate::value::FieldKind::Money);
            let class = if num { r#" class="z-num""# } else { "" };
            out.push_str(&format!(r#"<th scope="col"{class} data-field="{}" aria-sort="none""#, c.meta.name));
            if !style.is_empty() {
                out.push_str(&format!(r#" style="{style}""#));
            }
            if c.filter {
                out.push_str(r#" data-filter="true""#);
            }
            out.push_str(&format!(">{}</th>", r.label(c.meta)));
        }
        out.push_str("</tr></thead><tbody data-zeyn-rows>");
        if let Some(p) = page {
            out.push_str(&self.rows_html_erased(p, r));
        }
        out.push_str("</tbody>");
        if self.columns.iter().any(|c| c.sum) {
            out.push_str("<tfoot><tr>");
            for c in &self.columns {
                if c.sum {
                    out.push_str(&format!(r#"<td class="z-num" data-sum="{}"></td>"#, c.meta.name));
                } else {
                    out.push_str("<td></td>");
                }
            }
            out.push_str("</tr></tfoot>");
        }
        out.push_str("</table>");
        match self.paging {
            Paging::Pages { .. } => out.push_str(&format!(
                r#"<nav class="z-pager" aria-label="{}" data-testid="{scope}.pager"><button type="button" class="z-button" data-zeyn-page="prev" data-testid="{scope}.pager.prev" aria-label="{}" disabled>&lsaquo;</button><span class="z-page-info" aria-live="polite" data-zeyn-page-info></span><button type="button" class="z-button" data-zeyn-page="next" data-testid="{scope}.pager.next" aria-label="{}">&rsaquo;</button></nav>"#,
                grid_text("pagination"),
                grid_text("previous"),
                grid_text("next"),
            )),
            Paging::Infinite { .. } => out.push_str(&format!(
                r#"<div class="z-more" data-zeyn-more><button type="button" class="z-button" data-testid="{scope}.grid.load-more">{}</button><span class="z-sr-only" aria-live="polite" data-zeyn-page-info></span></div>"#,
                grid_text("load_more"),
            )),
            Paging::Virtual { .. } => out.push_str(&format!(
                r#"<p class="z-sr-only" aria-live="polite" data-zeyn-page-info data-testid="{scope}.grid.position"></p>"#
            )),
            Paging::None => {}
        }
        out.push_str("</div>");
    }

    fn rows_html_erased(&self, page: &LoadedPage, r: &Render) -> String {
        page.items.downcast_ref::<Vec<E>>().map(|rows| self.rows(rows, r)).unwrap_or_default()
    }
}

impl<E: Model> GridSource for Grid<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self) -> GridQuery {
        let page = match self.paging {
            Paging::Infinite { batch } => PageRequest::After { cursor: None, limit: batch },
            other => PageRequest::Offset { offset: 0, limit: other.size() },
        };
        GridQuery { page, with_total: !matches!(self.paging, Paging::Infinite { .. }), ..GridQuery::default() }
    }

    fn fetch(&self, ctx: Ctx, query: GridQuery) -> BoxFuture<Result<LoadedPage>> {
        let fetch = self.fetch;
        let offset = match &query.page {
            PageRequest::Offset { offset, .. } => *offset,
            PageRequest::After { .. } => 0,
        };
        Box::pin(async move {
            let page = fetch(ctx, query).await?;
            Ok(LoadedPage {
                count: page.items.len(),
                total: page.total,
                next: page.next,
                offset,
                items: Box::new(page.items),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        self.rows_html_erased(page, r)
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> serde_json::Value {
        let fields: Vec<&str> = self.columns.iter().map(|c| c.meta.name).collect();
        let filterable: Vec<&str> = self.columns.iter().filter(|c| c.filter).map(|c| c.meta.name).collect();
        json!({
            "type": "object",
            "properties": {
                "page": {
                    "type": "object",
                    "description": "Which rows to return. Use type offset with offset and limit (max 500).",
                    "properties": {
                        "type": { "type": "string", "enum": ["offset", "after"] },
                        "offset": { "type": "integer", "minimum": 0 },
                        "limit": { "type": "integer", "minimum": 1, "maximum": MAX_PAGE_SIZE },
                        "cursor": { "type": ["string", "null"] }
                    },
                    "required": ["type", "limit"]
                },
                "sort": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "field": { "type": "string", "enum": fields }, "desc": { "type": "boolean" } },
                        "required": ["field", "desc"]
                    }
                },
                "filters": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "field": { "type": "string", "enum": filterable }, "contains": { "type": "string" } },
                        "required": ["field", "contains"]
                    }
                },
                "with_total": { "type": "boolean" }
            }
        })
    }
}

impl<E: Model> Component for Grid<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "grid",
            "model": E::NAME,
            "action": self.action,
            "mode": self.mode,
            "paging": self.paging,
            "openable": self.on_open.is_some(),
            "columns": self.columns.iter().map(|c| json!({
                "field": c.meta.name,
                "label": c.meta.label,
                "kind": c.meta.kind,
                "width": c.width,
                "flex": c.flex,
                "filter": c.filter,
                "sum": c.sum,
            })).collect::<Vec<_>>(),
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }
}

/// Configures the last added column. Grid methods can be called from here too.
pub struct GridColumn<E: Model, T> {
    grid: Grid<E>,
    _type: PhantomData<fn() -> T>,
}

impl<E: Model, T> GridColumn<E, T> {
    fn last(&mut self) -> &mut Column<E> {
        self.grid.columns.last_mut().expect("GridColumn always has a column")
    }

    /// Fixed width in pixels.
    pub fn width(mut self, px: u32) -> Self {
        self.last().width = Some(px);
        self
    }

    /// Takes a share of the remaining width.
    pub fn flex(mut self, weight: u32) -> Self {
        self.last().flex = Some(weight);
        self
    }

    /// Adds a filter box to the column.
    pub fn filter(mut self) -> Self {
        self.last().filter = true;
        self
    }

    /// Shows the column total in the footer. Compiles only on numeric and money columns.
    pub fn sum(mut self) -> Self
    where
        T: SupportsSum,
    {
        self.last().sum = true;
        self
    }

    pub fn column<U: FieldType + 'static>(self, field: Field<E, U>) -> GridColumn<E, U> {
        self.grid.column(field)
    }

    pub fn mode(self, mode: GridMode) -> Grid<E> {
        self.grid.mode(mode)
    }

    pub fn paging(self, paging: Paging) -> Grid<E> {
        self.grid.paging(paging)
    }

    pub fn row_style(self, style: impl Fn(&E) -> Option<RowStyle> + Send + Sync + 'static) -> Grid<E> {
        self.grid.row_style(style)
    }

    pub fn on_open(self, open: impl Fn(&E) -> Effect + Send + Sync + 'static) -> Grid<E> {
        self.grid.on_open(open)
    }

    /// Returns the grid.
    pub fn done(self) -> Grid<E> {
        self.grid
    }
}

impl<E: Model, T> From<GridColumn<E, T>> for Grid<E> {
    fn from(c: GridColumn<E, T>) -> Self {
        c.grid
    }
}

impl<E: Model> From<Grid<E>> for Node {
    fn from(g: Grid<E>) -> Self {
        Node::new(g)
    }
}

impl<E: Model, T> From<GridColumn<E, T>> for Node {
    fn from(c: GridColumn<E, T>) -> Self {
        Node::new(c.grid)
    }
}

/// Text used to filter a value: what the user sees, lower cased.
fn filter_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Text(s) => s.to_lowercase(),
        Value::Money(m) => m.format_plain("en"),
        Value::Choice(c) => c.to_lowercase(),
        Value::Date(d) => d.to_string(),
        Value::Time(t) => t.to_string(),
        Value::DateTime(d) => d.to_string(),
        Value::LocalDateTime(l) => l.to_string(),
    }
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Money(x), Value::Money(y)) => x.minor.cmp(&y.minor),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| x.cmp(y)),
        (Value::Choice(x), Value::Choice(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::Time(x), Value::Time(y)) => x.cmp(y),
        (Value::DateTime(x), Value::DateTime(y)) => x.cmp(y),
        (Value::LocalDateTime(x), Value::LocalDateTime(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

impl GridQuery {
    /// Filters, sorts and pages an in memory list. Meant for small data, samples and tests;
    /// databases do this in SQL. Unknown field names are ignored. Sorting always ends with the
    /// key field so pages are stable.
    pub fn apply<M: Model>(&self, rows: Vec<M>) -> Page<M> {
        let fields = M::fields();
        let index = |name: &str| fields.iter().position(|f| f.name == name);
        let filters: Vec<(usize, String)> = self
            .filters
            .iter()
            .filter(|f| !f.contains.trim().is_empty())
            .filter_map(|f| index(&f.field).map(|i| (i, f.contains.trim().to_lowercase())))
            .collect();
        let mut rows: Vec<M> = rows
            .into_iter()
            .filter(|row| {
                filters.iter().all(|(i, needle)| {
                    row.value_at(*i).map(|v| filter_text(&v).contains(needle.as_str())).unwrap_or(false)
                })
            })
            .collect();
        let mut order: Vec<(usize, bool)> = self.sort.iter().filter_map(|s| index(&s.field).map(|i| (i, s.desc))).collect();
        if let Some(key) = fields.iter().position(|f| f.key) {
            order.push((key, false));
        }
        rows.sort_by(|a, b| {
            for (i, desc) in &order {
                let (x, y) = (a.value_at(*i).unwrap_or(Value::Null), b.value_at(*i).unwrap_or(Value::Null));
                let o = compare(&x, &y);
                let o = if *desc { o.reverse() } else { o };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        let mut page = Page::from_slice(&rows, self);
        if !self.with_total && matches!(self.page, PageRequest::After { .. }) {
            page.total = None;
        }
        page
    }
}

/// Cursor for a row position in an in memory list.
pub fn position_cursor(position: usize) -> Cursor {
    Cursor(position.to_string())
}
