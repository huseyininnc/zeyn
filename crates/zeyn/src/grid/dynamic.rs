//! [`DynamicGrid`]: a [`DataGrid`] whose columns come with its rows, for results whose shape is
//! known only once the query has run, such as SQL previews and data source browsers.
//!
//! The action answers a [`DataQuery`] with a [`DynamicPage`]. The grid turns its columns into
//! grid columns for that page only and renders the rows through the typed grid's own code, so
//! paging, virtual scrolling, keyboard use and formatting in the user's language are the same.

use super::*;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};

/// Most columns a dynamic page keeps; columns after it are dropped.
pub const MAX_DYNAMIC_COLUMNS: usize = 200;

/// Most characters of a dynamic column's name or label; longer ones are cut.
pub const MAX_COLUMN_NAME: usize = 200;

/// Most characters of a text cell of a dynamic page; longer text is cut and ends in `…`.
pub const MAX_CELL_TEXT: usize = 1_000;

/// How a dynamic column reads and shows its values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicKind {
    /// Any value, shown as text.
    #[default]
    Text,
    /// Whole numbers, grouped in the user's locale and right aligned.
    Integer,
    /// Exact decimal numbers (`1234.50` or a JSON number), in the user's locale.
    Decimal,
    /// `true`, `false`, `1` or `0`, shown as yes and no in the user's language.
    Bool,
    /// `2026-03-31`, or the date part of a longer value.
    Date,
    /// `14:30` or `14:30:05`.
    Time,
    /// An exact moment in RFC 3339 (`2026-03-31T11:30:00Z`), shown in the user's time zone.
    DateTime,
    /// A wall clock date and time without a zone (`2026-03-31T14:30:00`).
    LocalDateTime,
}

impl DynamicKind {
    fn meta(self) -> &'static FieldMeta {
        &KIND_META[self as usize]
    }

    /// Starting width of a column of this kind in pixels, so a wide result scrolls sideways
    /// instead of squeezing its columns; the user can resize it.
    fn width(self) -> u32 {
        match self {
            DynamicKind::Text => 180,
            DynamicKind::Integer | DynamicKind::Decimal => 130,
            DynamicKind::Bool | DynamicKind::Time => 90,
            DynamicKind::Date => 120,
            DynamicKind::DateTime | DynamicKind::LocalDateTime => 180,
        }
    }
}

const fn kind_meta(kind: FieldKind) -> FieldMeta {
    FieldMeta {
        model: "",
        name: "",
        label: "",
        kind,
        rules: &[],
        key: false,
        readonly: true,
        version: false,
        optional: true,
        mask: None,
    }
}

/// Field metadata of each [`DynamicKind`], in its order. The name and label of a dynamic column
/// live on the column itself.
static KIND_META: [FieldMeta; 8] = [
    kind_meta(FieldKind::Text),
    kind_meta(FieldKind::Integer),
    kind_meta(FieldKind::Decimal),
    kind_meta(FieldKind::Bool),
    kind_meta(FieldKind::Date),
    kind_meta(FieldKind::Time),
    kind_meta(FieldKind::DateTime),
    kind_meta(FieldKind::LocalDateTime),
];

/// One column of a [`DynamicPage`]: the name rows use, the header label and the kind.
///
/// The name and the label come from the action (a query, a data source) and are treated as user
/// data: escaped in the page and cut at [`MAX_COLUMN_NAME`] characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DynamicColumn {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub kind: DynamicKind,
}

impl DynamicColumn {
    /// Column `name` with the header `label`, read as `kind`.
    pub fn new(name: impl Into<String>, label: impl Into<String>, kind: DynamicKind) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind,
        }
    }
}

/// What the action of a [`DynamicGrid`] returns: the columns and one page of rows.
///
/// A row is a JSON array with one value per column, in column order, or a JSON object keyed by
/// column name. Values are read with the column's [`DynamicKind`]; a value that does not read
/// as its kind is shown as text.
///
/// Example:
/// ```
/// use zeyn::{DynamicColumn, DynamicKind, DynamicPage};
/// let page = DynamicPage::new(
///     vec![DynamicColumn::new("n", "Count", DynamicKind::Integer)],
///     vec![serde_json::json!([1234])],
/// )
/// .total(1);
/// assert_eq!(page.total, Some(1));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DynamicPage {
    pub columns: Vec<DynamicColumn>,
    pub rows: Vec<serde_json::Value>,
    /// Total row count, when the action knows it.
    pub total: Option<u64>,
    /// Where the next batch starts. `None` means there is nothing more.
    pub next: Option<Cursor>,
}

impl DynamicPage {
    /// A page with these columns and rows.
    pub fn new(columns: Vec<DynamicColumn>, rows: Vec<serde_json::Value>) -> Self {
        Self {
            columns,
            rows,
            total: None,
            next: None,
        }
    }

    /// Sets the total row count.
    pub fn total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }

    /// Sets where the next batch starts.
    pub fn next(mut self, cursor: Cursor) -> Self {
        self.next = Some(cursor);
        self
    }
}

/// One row of a dynamic grid: its cells, read with the kinds of the page's columns.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DynamicRow(Vec<Value>);

impl DynamicRow {
    /// The cells, in column order.
    pub fn cells(&self) -> &[Value] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DynamicRow {
    fn deserialize<D: serde::Deserializer<'de>>(_: D) -> std::result::Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "dynamic grid rows are rendered only and never read back",
        ))
    }
}

impl Model for DynamicRow {
    const NAME: &'static str = "DynamicRow";

    fn fields() -> &'static [FieldMeta] {
        &[]
    }

    fn value_at(&self, _index: usize) -> Option<Value> {
        None
    }

    fn validate(&self) -> std::result::Result<(), crate::model::ValidationErrors> {
        Ok(())
    }
}

/// A loaded dynamic page: the cleaned columns and the rows read with them.
pub(crate) struct DynamicItems {
    columns: Vec<DynamicColumn>,
    pub(crate) rows: Vec<DynamicRow>,
}

type DynamicFetch = fn(Ctx, DataQuery) -> BoxFuture<Result<DynamicPage>>;

fn fetch_dynamic<A>(ctx: Ctx, query: DataQuery) -> BoxFuture<Result<DynamicPage>>
where
    A: Action<Input = DataQuery, Output = Result<DynamicPage>>,
{
    Box::pin(async move { A::call(ctx, query).await? })
}

fn no_rows(_: Ctx, _: DataQuery) -> BoxFuture<Result<Page<DynamicRow>>> {
    Box::pin(async { Err(crate::action::Error::NotFound) })
}

/// Cuts `text` to `max` characters, ending in `…` when it was longer.
fn clip(mut text: String, max: usize) -> String {
    if text.len() > max
        && let Some((at, _)) = text.char_indices().nth(max)
    {
        text.truncate(at);
        text.push('…');
    }
    text
}

/// Reads one JSON value as a cell of `kind`. A value that does not read as the kind becomes
/// text, cut at [`MAX_CELL_TEXT`] characters.
fn cell(json: serde_json::Value, kind: DynamicKind) -> Value {
    use serde_json::Value as Json;
    let text = match json {
        Json::Null => return Value::Null,
        Json::Bool(b) if kind == DynamicKind::Bool => return Value::Bool(b),
        Json::Number(n) => match (kind, n.as_i64()) {
            (DynamicKind::Integer, Some(i)) => return Value::Int(i),
            (DynamicKind::Bool, Some(0 | 1)) => return Value::Bool(n.as_i64() == Some(1)),
            _ => n.to_string(),
        },
        Json::String(s) => s,
        other => other.to_string(),
    };
    let read = match kind {
        DynamicKind::Text => None,
        DynamicKind::Integer => text.trim().parse().ok().map(Value::Int),
        DynamicKind::Decimal => crate::Decimal::parse(&text).map(Value::Number),
        DynamicKind::Bool => match text.trim() {
            "true" | "t" | "1" => Some(Value::Bool(true)),
            "false" | "f" | "0" => Some(Value::Bool(false)),
            _ => None,
        },
        DynamicKind::Date => text
            .get(..10)
            .and_then(Date::parse)
            .map(Value::Date),
        DynamicKind::Time => Time::parse(&text).map(Value::Time),
        DynamicKind::DateTime => DateTime::parse(&text).map(Value::DateTime),
        DynamicKind::LocalDateTime => LocalDateTime::parse(&text).map(Value::LocalDateTime),
    };
    read.unwrap_or_else(|| Value::Text(clip(text, MAX_CELL_TEXT)))
}

/// Reads one JSON row (an array in column order or an object by name) into cells.
fn row(json: serde_json::Value, columns: &[DynamicColumn]) -> DynamicRow {
    let mut cells = Vec::with_capacity(columns.len());
    match json {
        serde_json::Value::Array(values) => {
            let mut values = values.into_iter();
            for c in columns {
                cells.push(cell(values.next().unwrap_or_default(), c.kind));
            }
        }
        serde_json::Value::Object(mut map) => {
            for c in columns {
                cells.push(cell(map.remove(&c.name).unwrap_or_default(), c.kind));
            }
        }
        _ => cells.resize(columns.len(), Value::Null),
    }
    DynamicRow(cells)
}

/// A short fingerprint of the columns, so the browser knows when the header must change.
fn shape(columns: &[DynamicColumn]) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    columns.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// DataGrid whose columns come from its action with every page. Built by [`DataGrid::dynamic`].
///
/// Example:
/// ```ignore
/// DataGrid::dynamic(preview_query).paging(Paging::virtual_scroll())
/// ```
pub struct DynamicGrid {
    grid: DataGrid<DynamicRow>,
    fetch: DynamicFetch,
}

impl DataGrid<DynamicRow> {
    /// DataGrid whose columns are known only after its action ran: the action takes a
    /// `DataQuery` and returns a [`DynamicPage`] with the columns and one page of rows. Paging,
    /// virtual scrolling, keyboard use, column layout and formatting by kind work as in a typed
    /// grid. Column names, labels and cells are escaped like any user data, at most
    /// [`MAX_DYNAMIC_COLUMNS`] columns and the query's page size of rows are kept, and long text
    /// is cut. Sort keys, filters and totals never reach the action. Rows are read only: no
    /// editing, row actions, selection or export.
    pub fn dynamic<A>(_source: A) -> DynamicGrid
    where
        A: Action<Input = DataQuery, Output = Result<DynamicPage>>,
    {
        let mut grid = DataGrid::of_fetch(A::META, no_rows);
        grid.paging = Paging::default();
        DynamicGrid {
            grid,
            fetch: fetch_dynamic::<A>,
        }
    }
}

impl DynamicGrid {
    /// How rows are paged; see [`Paging`].
    pub fn paging(mut self, paging: Paging) -> Self {
        self.grid.paging = paging;
        self
    }

    /// Shades every other row, for very wide results where the eye loses the row.
    pub fn zebra(mut self) -> Self {
        self.grid.zebra = true;
        self
    }

    /// The typed grid for a page's columns, with this grid's settings.
    fn shaped(&self, columns: &[DynamicColumn]) -> DataGrid<DynamicRow> {
        let mut grid = DataGrid::of_fetch(self.grid.action, no_rows);
        grid.paging = self.grid.paging;
        grid.zebra = self.grid.zebra;
        grid.shape = Some(shape(columns));
        for (i, c) in columns.iter().enumerate() {
            grid.columns.push(Column {
                name: Cow::Owned(c.name.clone()),
                label: Some(c.label.clone()),
                sort: false,
                width: Some(c.kind.width()),
                value: Box::new(move |row: &DynamicRow| {
                    row.0
                        .get(i)
                        .cloned()
                        .unwrap_or(Value::Null)
                }),
                ..Column::of(c.kind.meta())
            });
        }
        grid
    }

    fn columns(page: &LoadedPage) -> &[DynamicColumn] {
        page.items
            .downcast_ref::<DynamicItems>()
            .map_or(&[], |items| items.columns.as_slice())
    }
}

impl GridSource for DynamicGrid {
    fn action(&self) -> ActionMeta {
        self.grid.action
    }

    fn first_query(&self, view: Option<&DataGridView>) -> DataQuery {
        self.grid.first_query(view)
    }

    /// Keeps the layout of a view within the limits; columns the next page does not have are
    /// dropped when it renders. Sort keys and filters go, since the grid offers neither.
    fn clean_view(&self, view: DataGridView) -> DataGridView {
        DataGridView {
            columns: view
                .columns
                .into_iter()
                .filter(|c| c.field.chars().count() <= MAX_COLUMN_NAME)
                .take(MAX_DYNAMIC_COLUMNS)
                .map(|c| ColumnView {
                    width: c
                        .width
                        .map(|w| w.clamp(COLUMN_WIDTH.0, COLUMN_WIDTH.1)),
                    ..c
                })
                .collect(),
            sort: Vec::new(),
            filters: Vec::new(),
        }
    }

    fn head_html(&self, r: &Render, view: Option<&DataGridView>) -> String {
        self.grid.head_html(r, view)
    }

    fn page_head(
        &self,
        page: &LoadedPage,
        r: &Render,
        shape_seen: Option<&str>,
        asked: bool,
    ) -> Option<serde_json::Value> {
        let columns = Self::columns(page);
        let now = shape(columns);
        if !asked && shape_seen == Some(now.as_str()) {
            return None;
        }
        let grid = self.shaped(columns);
        Some(json!({
            "head": grid.head_html(r, page.view.as_ref()),
            "shape": now,
            "columns": grid.columns_json(r),
        }))
    }

    /// Calls the action with the page only: sort keys, filters, totals and groups are dropped
    /// first, the page size stays within [`MAX_PAGE_SIZE`]. The answer is cut to that many rows
    /// and [`MAX_DYNAMIC_COLUMNS`] columns, names and labels to [`MAX_COLUMN_NAME`] characters.
    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        query.sort.clear();
        query.filters.clear();
        query.aggregates.clear();
        query.group = None;
        query.time_zone = ctx.time_zone();
        let limit = query.limit() as usize;
        let offset = match &mut query.page {
            PageRequest::Offset { offset, limit: l } => {
                *l = limit as u32;
                *offset
            }
            PageRequest::After { limit: l, .. } => {
                *l = limit as u32;
                0
            }
        };
        let fetch = self.fetch;
        Box::pin(async move {
            let page = fetch(ctx, query).await?;
            let columns: Vec<DynamicColumn> = page
                .columns
                .into_iter()
                .take(MAX_DYNAMIC_COLUMNS)
                .map(|c| DynamicColumn {
                    name: clip(c.name, MAX_COLUMN_NAME),
                    label: clip(c.label, MAX_COLUMN_NAME),
                    kind: c.kind,
                })
                .collect();
            let rows: Vec<DynamicRow> = page
                .rows
                .into_iter()
                .take(limit)
                .map(|r| row(r, &columns))
                .collect();
            Ok(LoadedPage {
                count: rows.len(),
                total: page.total,
                next: page.next,
                offset,
                totals: Vec::new(),
                items: Box::new(DynamicItems { columns, rows }),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        self.shaped(Self::columns(page))
            .rows_html_erased(page, r)
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.shaped(Self::columns(page))
            .render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> serde_json::Value {
        self.grid.tool_schema()
    }
}

impl Component for DynamicGrid {
    fn render(&self, out: &mut String, r: &Render) {
        self.grid.render_with(out, r, None);
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "grid",
            "dynamic": true,
            "action": self.grid.action,
            "paging": self.grid.paging,
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }
}

impl From<DynamicGrid> for Node {
    fn from(g: DynamicGrid) -> Self {
        Node::new(g)
    }
}
