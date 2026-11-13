//! [`DynamicGrid`]: a [`DataGrid`] whose columns come with its rows, for results whose shape is
//! known only once the query has run, such as SQL previews and data source browsers.
//!
//! The action answers a [`DataQuery`] with a [`DynamicPage`]. The grid turns its columns into
//! grid columns for that page only and renders the rows through the typed grid's own code, so
//! paging, virtual scrolling, keyboard use and formatting in the user's language are the same.

use super::*;
use crate::privacy::MaskStyle;
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

/// Most cells one dynamic page keeps; rows after the budget wait for the next page.
pub const MAX_PAGE_CELLS: usize = 50_000;

/// Most bytes of text one dynamic page keeps over all its cells. Each text cell gets an equal
/// share (at least 64 bytes), so a wide page cuts its text shorter instead of dropping rows.
pub const MAX_PAGE_TEXT: usize = 8 * 1024 * 1024;

/// Raw values longer than this many bytes are masked completely instead of by their style.
const MAX_MASK_INPUT: usize = 1_024;

/// One column of a [`DynamicPage`]: the name rows use, the header label and the kind.
///
/// The name and the label come from the action (a query, a data source) and are treated as user
/// data: escaped in the page and cut at [`MAX_COLUMN_NAME`] characters. Empty names become
/// `col_<index>` and repeated names get `_2`, `_3`, so every column has a name of its own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DynamicColumn {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub kind: DynamicKind,
    /// Values are masked on the server before anything renders them; see
    /// [`DynamicColumn::masked`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<MaskStyle>,
}

impl DynamicColumn {
    /// Column `name` with the header `label`, read as `kind`.
    pub fn new(name: impl Into<String>, label: impl Into<String>, kind: DynamicKind) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            kind,
            mask: None,
        }
    }

    /// Shows the column's values masked with `style`, as text. The raw value is replaced as soon
    /// as the page is read, so it never reaches HTML, logs or AI agents, and no one can reveal
    /// it. Without this, keeping personal data out of the result is the action's job.
    ///
    /// ```
    /// use zeyn::{DynamicColumn, DynamicKind, MaskStyle};
    /// let iban = DynamicColumn::new("iban", "IBAN", DynamicKind::Text).masked(MaskStyle::LastFour);
    /// assert_eq!(iban.mask, Some(MaskStyle::LastFour));
    /// ```
    pub fn masked(mut self, style: MaskStyle) -> Self {
        self.mask = Some(style);
        self
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

/// A loaded dynamic page: the typed grid built once for its columns, and its rows.
pub(crate) struct DynamicItems {
    grid: DataGrid<DynamicRow>,
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

/// Cuts `text` to `max_chars` characters and `max_bytes` bytes, ending in `…` when it was
/// longer. A cut value is a new string, so the long one is freed.
fn clip(text: String, max_chars: usize, max_bytes: usize) -> String {
    if text.len() <= max_bytes && (text.len() <= max_chars || text.chars().nth(max_chars).is_none())
    {
        return text;
    }
    let mut end = 0;
    for (n, (i, c)) in text.char_indices().enumerate() {
        if n == max_chars || i + c.len_utf8() > max_bytes {
            break;
        }
        end = i + c.len_utf8();
    }
    let mut out = String::with_capacity(end + '…'.len_utf8());
    out.push_str(&text[..end]);
    out.push('…');
    out
}

/// Writes into a buffer and stops once it holds `cap` bytes, so serializing a huge value costs
/// no more than the part that is kept.
struct Capped {
    buf: Vec<u8>,
    cap: usize,
}

impl std::io::Write for Capped {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let room = self.cap.saturating_sub(self.buf.len());
        if room == 0 {
            return Err(std::io::Error::other("cell text is full"));
        }
        let n = data.len().min(room);
        self.buf.extend_from_slice(&data[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// JSON text of a nested value, at most `max_bytes` (plus a little to tell that it was longer).
fn json_text(value: &serde_json::Value, max_bytes: usize) -> String {
    let mut w = Capped {
        buf: Vec::new(),
        cap: max_bytes + 4,
    };
    let _ = serde_json::to_writer(&mut w, value);
    match String::from_utf8(w.buf) {
        Ok(s) => s,
        Err(e) => {
            let valid = e.utf8_error().valid_up_to();
            let mut bytes = e.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).unwrap_or_default()
        }
    }
}

/// How one column reads its values: the kind, the mask, the key an object row is read by and
/// whether a later column reads the same key.
struct Reader {
    kind: DynamicKind,
    mask: Option<MaskStyle>,
    key: String,
    shared: bool,
}

/// Reads one JSON value as a cell. A value that does not read as the kind becomes text, cut at
/// [`MAX_CELL_TEXT`] characters and `max_bytes` bytes; a masked column keeps only the masked
/// text.
fn cell(json: serde_json::Value, reader: &Reader, max_bytes: usize) -> Value {
    use serde_json::Value as Json;
    let kind = reader.kind;
    let text = match json {
        Json::Null => return Value::Null,
        Json::Bool(b) if kind == DynamicKind::Bool && reader.mask.is_none() => {
            return Value::Bool(b);
        }
        Json::Number(n) => match (kind, n.as_i64(), reader.mask) {
            (DynamicKind::Integer, Some(i), None) => return Value::Int(i),
            (DynamicKind::Bool, Some(i @ (0 | 1)), None) => return Value::Bool(i == 1),
            _ => n.to_string(),
        },
        Json::String(s) => s,
        other => json_text(&other, max_bytes),
    };
    if let Some(style) = reader.mask {
        return Value::Text(if text.len() > MAX_MASK_INPUT {
            MaskStyle::Full.apply("")
        } else {
            style.apply(&text)
        });
    }
    let read = match kind {
        _ if text.len() > 64 => None,
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
    read.unwrap_or_else(|| Value::Text(clip(text, MAX_CELL_TEXT, max_bytes)))
}

/// Reads one JSON row (an array in column order or an object by the action's column names)
/// into cells.
fn row(json: serde_json::Value, readers: &[Reader], max_bytes: usize) -> DynamicRow {
    let mut cells = Vec::with_capacity(readers.len());
    match json {
        serde_json::Value::Array(values) => {
            let mut values = values.into_iter();
            for r in readers {
                cells.push(cell(values.next().unwrap_or_default(), r, max_bytes));
            }
        }
        serde_json::Value::Object(mut map) => {
            for r in readers {
                let value = if r.shared {
                    map.get(&r.key).cloned()
                } else {
                    map.remove(&r.key)
                };
                cells.push(cell(value.unwrap_or_default(), r, max_bytes));
            }
        }
        _ => cells.resize(readers.len(), Value::Null),
    }
    DynamicRow(cells)
}

/// Bytes of text a cell holds, for the page's text budget.
fn text_bytes(row: &DynamicRow) -> usize {
    row.0
        .iter()
        .map(|v| match v {
            Value::Text(s) => s.len(),
            _ => 0,
        })
        .sum()
}

/// Cleans the action's columns: at most [`MAX_DYNAMIC_COLUMNS`], names and labels cut, empty
/// names replaced by `col_<index>` and repeated names made unique with `_2`, `_3`. Returns the
/// columns and how each reads its values (object rows by the action's own names).
fn clean_columns(columns: Vec<DynamicColumn>) -> (Vec<DynamicColumn>, Vec<Reader>) {
    let columns: Vec<DynamicColumn> = columns
        .into_iter()
        .take(MAX_DYNAMIC_COLUMNS)
        .collect();
    let mut readers: Vec<Reader> = Vec::with_capacity(columns.len());
    let mut seen = std::collections::HashSet::with_capacity(columns.len());
    let mut clean = Vec::with_capacity(columns.len());
    for (i, c) in columns.into_iter().enumerate() {
        let base = if c.name.trim().is_empty() {
            format!("col_{i}")
        } else {
            clip(c.name.clone(), MAX_COLUMN_NAME, MAX_COLUMN_NAME * 4)
        };
        let mut name = base.clone();
        let mut n = 2;
        while !seen.insert(name.clone()) {
            name = format!("{base}_{n}");
            n += 1;
        }
        if let Some(earlier) = readers
            .iter_mut()
            .rev()
            .find(|r| r.key == c.name)
        {
            earlier.shared = true;
        }
        readers.push(Reader {
            kind: c.kind,
            mask: c.mask,
            key: c.name,
            shared: false,
        });
        clean.push(DynamicColumn {
            name,
            label: clip(c.label, MAX_COLUMN_NAME, MAX_COLUMN_NAME * 4),
            kind: c.kind,
            mask: c.mask,
        });
    }
    (clean, readers)
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
    /// Model name and key column of audited reads, set by [`DynamicGrid::sensitive`].
    sensitive: Option<(&'static str, &'static str)>,
}

impl DataGrid<DynamicRow> {
    /// DataGrid whose columns are known only after its action ran: the action takes a
    /// `DataQuery` and returns a [`DynamicPage`] with the columns and one page of rows. Paging,
    /// virtual scrolling, keyboard use, column layout and formatting by kind work as in a typed
    /// grid. Column names, labels and cells are escaped like any user data. At most
    /// [`MAX_DYNAMIC_COLUMNS`] columns, the query's page size of rows, [`MAX_PAGE_CELLS`] cells
    /// and [`MAX_PAGE_TEXT`] bytes of text are kept; rows past a budget come with the next page.
    /// Sort keys, filters and totals never reach the action. Rows are read only: no editing, row
    /// actions, selection or export. Mask columns with [`DynamicColumn::masked`] and audit reads
    /// with [`DynamicGrid::sensitive`]; otherwise the action decides what the result may show.
    pub fn dynamic<A>(_source: A) -> DynamicGrid
    where
        A: Action<Input = DataQuery, Output = Result<DynamicPage>>,
    {
        let mut grid = DataGrid::of_fetch(A::META, no_rows);
        grid.paging = Paging::default();
        DynamicGrid {
            grid,
            fetch: fetch_dynamic::<A>,
            sensitive: None,
        }
    }
}

/// The typed grid for a page's columns, with a dynamic grid's settings.
fn shaped(
    action: ActionMeta,
    paging: Paging,
    zebra: bool,
    columns: &[DynamicColumn],
) -> DataGrid<DynamicRow> {
    let mut grid = DataGrid::of_fetch(action, no_rows);
    grid.paging = paging;
    grid.zebra = zebra;
    grid.shape = Some(shape(columns));
    for (i, c) in columns.iter().enumerate() {
        let kind = if c.mask.is_some() {
            DynamicKind::Text
        } else {
            c.kind
        };
        grid.columns.push(Column {
            name: Cow::Owned(c.name.clone()),
            label: Some(c.label.clone()),
            sort: false,
            width: Some(kind.width()),
            value: Box::new(move |row: &DynamicRow| match row.0.get(i) {
                Some(v) => Cow::Borrowed(v),
                None => Cow::Owned(Value::Null),
            }),
            ..Column::of(kind.meta())
        });
    }
    grid
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

    /// Audits every row the user is shown as a read of `model`, keyed by the value of the
    /// column `key` (the action's column name), as a `#[entity(sensitive)]` model's rows are.
    pub fn sensitive(mut self, model: &'static str, key: &'static str) -> Self {
        self.sensitive = Some((model, key));
        self
    }

    fn items(page: &LoadedPage) -> Option<&DynamicItems> {
        page.items
            .downcast_ref::<DynamicItems>()
    }
}

impl GridSource for DynamicGrid {
    fn action(&self) -> ActionMeta {
        self.grid.action
    }

    fn first_query(&self, view: Option<&DataGridView>) -> DataQuery {
        self.grid.first_query(view)
    }

    /// Keeps the layout of a view and the shape it was made for within the limits; columns the
    /// next page does not have are dropped when it renders, and a view of other columns is not
    /// used. Sort keys and filters go, since the grid offers neither.
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
            shape: view.shape.filter(|s| s.len() <= 32),
            group_by: Vec::new(),
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
        let grid = &Self::items(page)?.grid;
        let now = grid.shape.as_deref()?;
        if !asked && shape_seen == Some(now) {
            return None;
        }
        Some(json!({
            "head": grid.head_html(r, page.view.as_ref()),
            "shape": now,
            "columns": grid.columns_json(r),
        }))
    }

    /// Calls the action with the page only: sort keys, filters, totals and groups are dropped
    /// first, the page size stays within [`MAX_PAGE_SIZE`]. The answer is cut to that many rows,
    /// [`MAX_DYNAMIC_COLUMNS`] columns, [`MAX_PAGE_CELLS`] cells and [`MAX_PAGE_TEXT`] bytes of
    /// text; when a budget cuts rows, the page says where the next one starts.
    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        query.sort.clear();
        query.filters.clear();
        query.q.clear();
        query.aggregates.clear();
        query.group = None;
        query.time_zone = ctx.time_zone();
        let limit = query.limit() as usize;
        let start = query.span().offset;
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
        let (fetch, action, paging, zebra) = (
            self.fetch,
            self.grid.action,
            self.grid.paging,
            self.grid.zebra,
        );
        let sensitive = self.sensitive;
        let audit = ctx.clone();
        Box::pin(async move {
            let page = fetch(ctx, query).await?;
            let (columns, readers) = clean_columns(page.columns);
            let width = columns.len().max(1);
            let max_bytes = (MAX_PAGE_TEXT / (limit * width)).clamp(64, MAX_CELL_TEXT * 4);
            let max_rows = limit.min(MAX_PAGE_CELLS / width);
            let mut rows: Vec<DynamicRow> = Vec::with_capacity(max_rows.min(page.rows.len()));
            let mut text = 0;
            let mut cut = false;
            for json in page.rows {
                if rows.len() == max_rows || text >= MAX_PAGE_TEXT {
                    cut = true;
                    break;
                }
                let r = row(json, &readers, max_bytes);
                text += text_bytes(&r);
                rows.push(r);
            }
            cut &= rows.len() < limit;
            if let Some((model, key)) = sensitive {
                let at = readers
                    .iter()
                    .position(|r| r.key == key);
                for r in &rows {
                    let id = at
                        .and_then(|i| r.0.get(i))
                        .and_then(value_key);
                    audit.note_read(crate::audit::RecordRef { model, id });
                }
            }
            let grid = shaped(action, paging, zebra, &columns);
            Ok(LoadedPage {
                count: rows.len(),
                total: page.total,
                next: match cut {
                    true => Some(Cursor(
                        start
                            .saturating_add(rows.len() as u64)
                            .to_string(),
                    )),
                    false => page.next,
                },
                offset,
                totals: Vec::new(),
                items: Box::new(DynamicItems { grid, rows }),
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
        Self::items(page)
            .map(|items| items.grid.rows_html_erased(page, r))
            .unwrap_or_default()
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        match Self::items(page) {
            Some(items) => items
                .grid
                .render_with(out, r, Some(page)),
            None => self
                .grid
                .render_with(out, r, Some(page)),
        }
    }

    fn sensitive_model(&self) -> Option<&'static str> {
        self.sensitive.map(|s| s.0)
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
