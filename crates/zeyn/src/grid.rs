use crate::action::{
    Action, ActionMeta, Aggregate, AggregateRequest, Cursor, DEFAULT_PAGE_SIZE, DataQuery, Filter,
    MAX_FILTER_CHOICES, MAX_FILTER_TEXT, MAX_FILTERS, MAX_PAGE_SIZE, Page, PageRequest, Result,
    Sort, Total,
};
use crate::ctx::Outcome;
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::model::{Field, FieldMeta, Model, Valid};
use crate::render::{Render, attr, escape, escape_into};
use crate::screen::Icon;
use crate::screen::RowCall;
use crate::screen::{Component, Effect, GridSource, LoadedPage, Node, ScreenDef};
use crate::time::{Date, DateTime, LocalDateTime, Time, TimeZone};
use crate::value::{
    ChoiceType, FieldKind, FieldType, IsBool, IsMoment, Money, SupportsMinMaxTotal, SupportsSum,
    Value,
};
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

mod column;
mod dynamic;
mod memory;

pub use column::DataGridColumn;
pub use dynamic::{
    DynamicColumn, DynamicGrid, DynamicKind, DynamicPage, DynamicRow, MAX_CELL_TEXT,
    MAX_COLUMN_NAME, MAX_DYNAMIC_COLUMNS, MAX_PAGE_CELLS, MAX_PAGE_TEXT,
};

#[cfg(any(feature = "postgres", feature = "oracle"))]
pub(crate) use memory::{Cond, Key, TextOp};
pub use memory::{position_cursor, total};

/// Where sorting and filtering happen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataMode {
    /// Data comes to the client once; sorting and filtering run in the browser.
    #[default]
    Client,
    /// Every page, sort and filter is requested from the server. For large data.
    Server,
}

impl DataMode {
    fn code(self) -> &'static str {
        match self {
            DataMode::Client => "client",
            DataMode::Server => "server",
        }
    }
}

/// Rows of a virtual grid's first block rendered live in the page (enough for a tall screen
/// plus the overscan); the rest of the block waits in a `<template>` until it scrolls into view.
pub const VIRTUAL_LIVE_ROWS: usize = 64;

/// Fewest columns (with the selection, detail and row action columns) for which a virtual grid
/// sends its whole first block in the `<template>`, so the browser styles and lays out only the
/// columns the runtime keeps in view. Matches `COLV_MIN` of the browser runtime
/// (`assets/zeyn/grid-cells.js`), where column windowing starts.
pub const VIRTUAL_WIDE_COLUMNS: usize = 40;

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
        Paging::Pages {
            size: DEFAULT_PAGE_SIZE,
        }
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
        Paging::Pages {
            size: bounded(size),
        }
    }

    /// Infinite scroll loading `batch` rows at a time.
    pub const fn infinite(batch: u32) -> Self {
        Paging::Infinite {
            batch: bounded(batch),
        }
    }

    /// No paging.
    pub const fn none() -> Self {
        Paging::None
    }

    /// Virtual scrolling with rows of the user's density (40 px comfortable, 32 px compact),
    /// fetched in blocks of 200. For very large lists. The
    /// server sends the first block; rows after [`VIRTUAL_LIVE_ROWS`] go in an inert `<template>`
    /// so the browser styles and lays out only what fits on a screen. A grid with
    /// [`VIRTUAL_WIDE_COLUMNS`] columns or more puts every row there and the runtime inserts the
    /// rows once it has chosen the columns in view.
    pub const fn virtual_scroll() -> Self {
        Paging::Virtual {
            row_height: 0,
            block: 200,
        }
    }

    /// Virtual scrolling with a custom row height in pixels.
    pub const fn virtual_rows(row_height: u32) -> Self {
        Paging::Virtual {
            row_height: if row_height < 20 { 20 } else { row_height },
            block: 200,
        }
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

/// Side a column is pinned to. `Start` is the left in left to right languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pin {
    Start,
    End,
}

impl Pin {
    fn code(self) -> &'static str {
        match self {
            Pin::Start => "start",
            Pin::End => "end",
        }
    }
}

/// Most columns pinned to one side.
pub const MAX_PINNED: usize = 3;
/// Narrowest and widest column a user may set, in pixels.
pub const COLUMN_WIDTH: (u32, u32) = (48, 1200);

/// One shown column of a view: field, width set by the user and pin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnView {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pin>,
}

/// How a user arranged a grid: shown columns in order with widths and pins, sort and filters.
/// Comes from the client, so the grid cleans it with its own columns before any use.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DataGridView {
    /// Shown columns in order. Empty means the grid's own layout.
    #[serde(default)]
    pub columns: Vec<ColumnView>,
    #[serde(default)]
    pub sort: Vec<Sort>,
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// Fingerprint of the columns of a dynamic grid the view was made for. A dynamic grid uses
    /// a view only for the same columns; typed grids ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

/// Where saved grid views live, one per user and grid. Register one with `App::datagrid_views`.
///
/// `grid` is a key built by the server from the tenant, the screen and the node; `user` is the
/// signed in user. Implementations store the view as JSON in a table or a key value store.
pub trait DataGridViewStore: Send + Sync + 'static {
    fn load(&self, user: &str, grid: &str) -> BoxFuture<Result<Option<DataGridView>>>;

    /// Saves a view, or deletes it when `view` is `None`.
    fn save(&self, user: &str, grid: &str, view: Option<DataGridView>) -> BoxFuture<Result<()>>;
}

/// Saved views in memory, lost on restart. For development, tests and samples.
#[derive(Clone, Default)]
pub struct MemoryViewStore(Arc<Mutex<HashMap<(String, String), DataGridView>>>);

impl DataGridViewStore for MemoryViewStore {
    fn load(&self, user: &str, grid: &str) -> BoxFuture<Result<Option<DataGridView>>> {
        let found = self
            .0
            .lock()
            .map(|m| {
                m.get(&(user.to_string(), grid.to_string()))
                    .cloned()
            })
            .map_err(poisoned);
        Box::pin(async move { found })
    }

    fn save(&self, user: &str, grid: &str, view: Option<DataGridView>) -> BoxFuture<Result<()>> {
        let key = (user.to_string(), grid.to_string());
        let done = self.0.lock().map(|mut m| match view {
            Some(v) => {
                m.insert(key, v);
            }
            None => {
                m.remove(&key);
            }
        });
        let done = done.map_err(poisoned);
        Box::pin(async move { done })
    }
}

fn poisoned<T>(_: T) -> crate::action::Error {
    crate::action::Error::Internal("view store lock is poisoned".into())
}

/// A column as shown in one render: which column, its width and its pin.
struct Shown {
    index: usize,
    width: Option<u32>,
    pin: Option<Pin>,
}

/// Color of a badge. Tones follow the theme, so they work in light and dark mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
    Brand,
}

impl Tone {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Tone::Neutral => "neutral",
            Tone::Info => "info",
            Tone::Success => "success",
            Tone::Warning => "warning",
            Tone::Danger => "danger",
            Tone::Brand => "brand",
        }
    }
}

/// Draws the content of a grid cell from the column's typed value. The extension point for
/// app specific cells; closures `Fn(&T, &Render) -> String` are renderers too.
///
/// The returned HTML goes into the cell as is: escape user data with [`crate::render::escape`]
/// or [`Render::value`]. Exports and copy use the plain value, not this HTML.
///
/// ```ignore
/// DataGrid::of(search_products).column(Product::code).cell(|code: &String, _r: &Render| {
///     format!("<code>{}</code>", zeyn::render::escape(code))
/// })
/// ```
pub trait CellRenderer<T>: Send + Sync + 'static {
    fn render(&self, value: &T, r: &Render) -> String;
}

impl<T, F> CellRenderer<T> for F
where
    F: Fn(&T, &Render) -> String + Send + Sync + 'static,
{
    fn render(&self, value: &T, r: &Render) -> String {
        self(value, r)
    }
}

type CellFn<E> = Box<dyn Fn(&E, &Render) -> String + Send + Sync>;
type RunFn<E> =
    Box<dyn Fn(&E) -> Box<dyn FnOnce(Ctx) -> BoxFuture<Result<()>> + Send> + Send + Sync>;

/// Most rows one bulk action may cover; larger selections are refused.
pub const MAX_BULK: usize = 10_000;

/// Actions that take the keys of selected rows: their input is `Vec<K>`, where `K` is the type of
/// the key field given to `bulk_action`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be a bulk action over `{K}` keys",
    label = "this action's input is not `Vec<{K}>`",
    note = "a bulk action takes the selected keys: declare its first argument as `Vec<{K}>`, the type of the key field you pass"
)]
pub trait BulkAction<K>: Action {}

impl<K, A: Action<Input = Vec<K>>> BulkAction<K> for A {}

type BulkFn = Box<
    dyn Fn(Vec<serde_json::Value>) -> Option<Box<dyn FnOnce(Ctx) -> BoxFuture<Result<()>> + Send>>
        + Send
        + Sync,
>;

/// A button in the selection bar that runs an action with the selected keys.
struct BulkDef {
    label: Text,
    meta: ActionMeta,
    run: BulkFn,
    ask: Ask,
}

/// What a row or bulk action asks before it runs: a confirmation message, and whether it is
/// drawn as destructive.
#[derive(Default)]
struct Ask {
    confirm: Option<Text>,
    danger: bool,
}

impl Ask {
    /// The ` data-zeyn-confirm="..."` attribute of the button, or nothing.
    fn attr(&self, r: &Render) -> String {
        self.confirm
            .as_ref()
            .map(|m| format!(r#" data-zeyn-confirm="{}""#, r.text(m)))
            .unwrap_or_default()
    }
}

/// A button on every row that runs an action with input built from the row.
struct RowAction<E> {
    label: Text,
    icon: Option<Icon>,
    meta: ActionMeta,
    run: RunFn<E>,
    ask: Ask,
}

/// Runs action `A` with `input` and turns a failed outcome into an error.
fn runner<A>(input: A::Input) -> Box<dyn FnOnce(Ctx) -> BoxFuture<Result<()>> + Send>
where
    A: Action,
    A::Output: Outcome,
{
    Box::new(move |ctx| {
        Box::pin(async move {
            let output = A::call(ctx, input).await?;
            output.failure().map_or(Ok(()), Err)
        })
    })
}

/// Key of a row as text: the value of the model's `#[key]` field.
/// Appends [`key_text`] escaped for an attribute, without a new string for numbers and text.
fn key_into(out: &mut String, value: &Value) {
    match value {
        Value::Int(n) => {
            use std::fmt::Write;
            let _ = write!(out, "{n}");
        }
        Value::Text(s) => escape_into(out, s),
        other => escape_into(out, &key_text(other)),
    }
}

fn key_text(value: &Value) -> String {
    match value {
        Value::Int(n) => n.to_string(),
        Value::Text(s) => s.clone(),
        Value::Choice(c) => c.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

type ValueFn<E> = Box<dyn for<'a> Fn(&'a E) -> std::borrow::Cow<'a, Value> + Send + Sync>;
type StyleFn<E> = Box<dyn Fn(&E) -> Option<RowStyle> + Send + Sync>;
type OpenFn<E> = Box<dyn Fn(&E) -> Effect + Send + Sync>;
pub(crate) type FetchFn<E> = fn(Ctx, DataQuery) -> BoxFuture<Result<Page<E>>>;

struct Column<E> {
    meta: &'static FieldMeta,
    /// Field name the client and views use: the field's own, or a dynamic column's.
    name: std::borrow::Cow<'static, str>,
    /// Header label as given, for dynamic columns; `None` uses the field's label text.
    label: Option<String>,
    value: ValueFn<E>,
    width: Option<u32>,
    flex: Option<u32>,
    filter: bool,
    sort: bool,
    aggregates: Vec<Aggregate>,
    pin: Option<Pin>,
    hidden: bool,
    cell: Option<CellFn<E>>,
    editable: bool,
    /// Whole numbers shown without group separators, for codes and years.
    plain: bool,
    /// Long values wrap onto more lines instead of ending in an ellipsis.
    wrap: bool,
}

impl<E> Column<E> {
    /// Column of `meta` with every option off, showing no value until `value` is set.
    fn of(meta: &'static FieldMeta) -> Self {
        Column {
            meta,
            name: std::borrow::Cow::Borrowed(meta.name),
            label: None,
            value: Box::new(|_| std::borrow::Cow::Owned(Value::Null)),
            width: None,
            flex: None,
            filter: false,
            sort: true,
            aggregates: Vec::new(),
            pin: None,
            hidden: false,
            cell: None,
            editable: false,
            plain: false,
            wrap: false,
        }
    }

    /// Header label in the user's language, not escaped.
    fn label_plain(&self, r: &Render) -> String {
        match &self.label {
            Some(label) => label.clone(),
            None => r
                .i18n
                .text(r.locale, &self.meta.label_text()),
        }
    }
}

/// Data grid. Its data source is an action that takes a `DataQuery` and returns `Page<E>`.
///
/// Example:
/// ```ignore
/// DataGrid::of(search_products).column(Product::code).width(120).filter()
/// ```
pub struct DataGrid<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    mode: DataMode,
    paging: Paging,
    columns: Vec<Column<E>>,
    row_style: Option<StyleFn<E>>,
    on_open: Option<OpenFn<E>>,
    row_actions: Vec<RowAction<E>>,
    selectable: bool,
    zebra: bool,
    bulk: Vec<BulkDef>,
    /// Whether the last added action is a bulk action, the one `confirm` and `tone` set.
    last_bulk: bool,
    row_menu: Option<MenuFn<E>>,
    save: Option<SaveDef<E>>,
    export: Option<(crate::action::Access, usize)>,
    detail: Option<DetailDef<E>>,
    scope: Vec<Filter>,
    default_sort: Vec<crate::action::Sort>,
    group_by: Vec<&'static FieldMeta>,
    tree: Option<&'static FieldMeta>,
    /// Fingerprint of a dynamic grid's columns, sent to the browser.
    shape: Option<String>,
}

/// Most levels a tree grid opens.
pub const MAX_TREE_DEPTH: usize = 32;

/// Where rows sit in a grouped or tree grid.
#[derive(Default)]
struct Branch<'a> {
    depth: usize,
    path: &'a [Option<String>],
    parents: &'a [String],
}

type ParamsFn<E> = Box<dyn Fn(&E) -> serde_json::Value + Send + Sync>;

/// The screen a row expands into and how its params are built from the row.
struct DetailDef<E> {
    screen: &'static str,
    access: crate::action::Access,
    params: ParamsFn<E>,
}

/// A value in the fixed machine form group paths and scopes use: numbers and money as `1234.56`,
/// dates and times in ISO form, choices by value. `None` for an empty value.
pub(crate) fn value_key(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Money(m) => {
            let a = m.minor.unsigned_abs();
            Some(format!(
                "{}{}.{:02}",
                if m.minor < 0 { "-" } else { "" },
                a / 100,
                a % 100
            ))
        }
        other => Some(key_text(other)),
    }
}

/// Filter that keeps rows whose `meta` field is one of `values`, or `None` when there are none,
/// so an empty choice keeps every row.
pub(crate) fn any_filter<T: FieldType>(
    meta: &FieldMeta,
    values: impl IntoIterator<Item = T>,
) -> Option<Filter> {
    let keys: Vec<String> = values
        .into_iter()
        .filter_map(|v| value_key(&v.to_value()))
        .take(crate::action::MAX_FILTER_CHOICES)
        .collect();
    (!keys.is_empty()).then(|| Filter::any_of(meta.name, keys))
}

/// Filter that keeps rows whose `meta` field is between `from` and `to`, both inclusive, or
/// `None` when both are open.
pub(crate) fn range_filter<T: FieldType>(
    meta: &FieldMeta,
    from: Option<T>,
    to: Option<T>,
) -> Option<Filter> {
    let from = from.and_then(|v| value_key(&v.to_value()));
    let to = to.and_then(|v| value_key(&v.to_value()));
    (from.is_some() || to.is_some())
        .then(|| Filter::range(meta.name, from.as_deref(), to.as_deref()))
}

/// Filter that keeps rows whose `meta` field equals `key` (from [`value_key`]), or is empty when
/// `key` is `None`.
pub(crate) fn key_filter(meta: &FieldMeta, key: Option<&str>) -> Filter {
    let Some(key) = key else {
        return Filter::blank(meta.name, true);
    };
    match meta.kind {
        FieldKind::Text | FieldKind::Custom { .. } => Filter {
            field: meta.name.to_string(),
            equals: Some(key.to_string()),
            ..Filter::default()
        },
        FieldKind::Choice(_) => Filter::any_of(meta.name, [key]),
        FieldKind::Bool => Filter::is(meta.name, key == "true"),
        _ => Filter::range(meta.name, Some(key), Some(key)),
    }
}

/// One column of a typed export: the label in the user's language and the width in pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportColumn {
    pub label: String,
    pub width: Option<u32>,
    /// Whether the column holds numbers or money, which printed exports align right.
    pub numeric: bool,
}

/// Most rows an export holds unless the grid sets another limit.
pub const DEFAULT_EXPORT_LIMIT: usize = 100_000;

/// Quotes one CSV cell (RFC 4180) and defuses spreadsheet formulas in text that is not a number:
/// at the start and after every character a spreadsheet may split cells on (`,`, `;`, tab), since
/// the list separator depends on the reader's locale.
fn csv_cell(value: &str) -> String {
    let numeric = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_digit() || ".,- +".contains(c))
    };
    let formula = |s: &str| s.starts_with(['=', '+', '-', '@', '\t', '\r']) && !numeric(s);
    let mut v = String::with_capacity(value.len() + 2);
    let mut rest = value;
    loop {
        if formula(rest) {
            v.push('\'');
        }
        match rest.find([',', ';', '\t']) {
            Some(i) => {
                v.push_str(&rest[..=i]);
                rest = &rest[i + 1..];
            }
            None => {
                v.push_str(rest);
                break;
            }
        }
    }
    if v.contains([',', ';', '\t', '"', '\r', '\n']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// The masked text of a masked field's value, as its grid cell shows it.
fn masked(meta: &FieldMeta, value: &Value) -> String {
    crate::audit::plain(value)
        .map(|v| crate::privacy::masked_text(meta, &v))
        .unwrap_or_default()
}

/// A value as the user reads it, escaped for HTML.
fn plain_html(value: &Value, kind: &FieldKind, r: &Render) -> String {
    escape(&plain(value, kind, r))
}

/// A value as plain text the user reads: money with the symbol, dates in the locale, choices by
/// label, yes and no in the user's language.
fn plain(value: &Value, kind: &FieldKind, r: &Render) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => r.i18n.text(
            r.locale,
            &Text::key(if *b { "zeyn.yes" } else { "zeyn.no" }),
        ),
        Value::Int(n) => n.to_string(),
        Value::Text(s) if matches!(kind, FieldKind::Choices(_)) => {
            crate::ui::choices_text(s, kind, r)
        }
        Value::Text(s) => s.clone(),
        Value::Money(m) => m.format(r.locale),
        Value::Date(d) => d.format(r.locale),
        Value::Time(t) => t.format(r.locale),
        Value::LocalDateTime(l) => l.format(r.locale),
        Value::DateTime(d) => d.format(r.tz, r.locale),
        Value::Decimal(x) => format!("{x:.2}"),
        Value::Number(d) => d.format(r.locale),
        Value::Choice(c) => match kind {
            FieldKind::Choice(options) => options
                .iter()
                .find(|o| o.value == *c)
                .map(|o| {
                    r.i18n
                        .text(r.locale, &Text::key_or(o.key, o.label))
                })
                .unwrap_or_else(|| c.to_string()),
            _ => c.to_string(),
        },
    }
}

type SaveRun<E> =
    Box<dyn Fn(Valid<E>) -> Box<dyn FnOnce(Ctx) -> BoxFuture<Result<()>> + Send> + Send + Sync>;

/// The action that saves inline edits.
struct SaveDef<E: Model> {
    meta: ActionMeta,
    run: SaveRun<E>,
}

type MenuFn<E> = Box<dyn Fn(&E) -> crate::menu::ContextMenu + Send + Sync>;

pub(crate) fn fetch_via<A, E>(ctx: Ctx, query: DataQuery) -> BoxFuture<Result<Page<E>>>
where
    A: Action<Input = DataQuery, Output = Result<Page<E>>>,
    E: Model,
{
    Box::pin(async move { A::call(ctx, query).await? })
}

impl<E: Model> DataGrid<E> {
    /// DataGrid that loads its rows from the given action.
    pub fn of<A>(_source: A) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<Page<E>>>,
    {
        Self::of_fetch(A::META, fetch_via::<A, E>)
    }

    /// DataGrid of an action's access rule and the function that loads its rows.
    fn of_fetch(action: ActionMeta, fetch: FetchFn<E>) -> Self {
        Self {
            action,
            fetch,
            mode: DataMode::default(),
            paging: Paging::default(),
            columns: Vec::new(),
            row_style: None,
            on_open: None,
            row_actions: Vec::new(),
            selectable: false,
            zebra: false,
            bulk: Vec::new(),
            last_bulk: false,
            row_menu: None,
            save: None,
            export: None,
            detail: None,
            scope: Vec::new(),
            default_sort: Vec::new(),
            group_by: Vec::new(),
            tree: None,
            shape: None,
        }
    }

    /// Groups rows by `field` on the server; call again to group inside each group. The grid
    /// shows a header row per group with its value, row count and the column totals over the
    /// group, and loads a group's rows (or subgroups) when the user opens it, a page at a time.
    /// The top level pages over groups. The grid's action answers `DataQuery::group` with
    /// `Page::groups`; `DataQuery::apply` does it for in memory data. Virtual grids do not group.
    /// Masked fields never group a grid, because a group shows its value: such a call is ignored
    /// and logged.
    pub fn group_by<T: FieldType>(mut self, field: Field<E, T>) -> Self {
        let meta = field.meta();
        if meta.mask.is_some() {
            tracing::warn!(
                "grid of `{}` cannot group by the masked field `{}`; ignored",
                E::NAME,
                meta.name
            );
            return self;
        }
        if !self
            .group_by
            .iter()
            .any(|m| m.name == meta.name)
        {
            self.group_by.push(meta);
        }
        self
    }

    /// Shows rows as a tree by their parent key: rows whose `parent` is empty at the top, and the
    /// children of a row loaded when the user opens it, a page at a time, to any depth (at most
    /// [`MAX_TREE_DEPTH`]). The parent field must be optional; empty means a top row. Filters and
    /// sorting apply at every level, totals cover every matching row. Tree grids do not group
    /// and show no row details; virtual grids show no tree.
    pub fn tree<K: FieldType>(mut self, parent: Field<E, Option<K>>) -> Self
    where
        Option<K>: FieldType,
    {
        self.tree = Some(parent.meta());
        self
    }

    /// Whether rows are grouped or in a tree.
    fn nested(&self) -> bool {
        !matches!(self.paging, Paging::Virtual { .. })
            && (self.tree.is_some() || !self.group_by.is_empty())
    }

    /// Lets every row expand into a screen built from the row, loaded only when the user opens
    /// it: `.detail(|o: &Order| (order_lines, OrderParams { id: o.id }))`. Params of the wrong type
    /// do not compile. The detail can hold anything a screen holds, such as another grid with its
    /// own detail, to any depth. The expander shows only for users who may open the screen, and
    /// the server checks the screen's permission again when it loads, as for any screen. The
    /// params travel through the browser like a window's, so the detail screen checks that the
    /// user may see the record they name.
    ///
    /// Virtual grids (`Paging::Virtual`) keep every row at one fixed height and show no expander;
    /// use pages or infinite paging for grids with details.
    pub fn detail<S: ScreenDef>(
        mut self,
        detail: impl Fn(&E) -> (S, S::Params) + Send + Sync + 'static,
    ) -> Self {
        self.detail = Some(DetailDef {
            screen: S::META.name,
            access: S::META.access,
            params: Box::new(move |row| {
                serde_json::to_value(detail(row).1).unwrap_or(serde_json::Value::Null)
            }),
        });
        self
    }

    /// The order rows come in before the user sorts: `.sort_by(Order::id, true)` for newest
    /// first. The header shows it, and clicking that column only switches between ascending and
    /// descending; other columns go back to this order on their third click. Call it again to add
    /// further keys. The action still receives it in `DataQuery::sort`.
    pub fn sort_by<T: FieldType>(mut self, field: Field<E, T>, desc: bool) -> Self {
        self.default_sort
            .push(crate::action::Sort {
                field: field.meta().name.to_string(),
                desc,
            });
        self
    }

    /// Keeps only the rows whose `field` equals `value`, on every request, whatever the client
    /// sends: `.scope(OrderLine::order_id, params.id)` for the lines of one order. The filter goes
    /// first in `DataQuery::filters`, so a data layer that reads the filters keeps it. Text
    /// compares ignoring case.
    pub fn scope<T: FieldType>(mut self, field: Field<E, T>, value: T) -> Self {
        self.scope.push(key_filter(
            field.meta(),
            value_key(&value.to_value()).as_deref(),
        ));
        self
    }

    /// Keeps only the rows whose `field` is one of `values`, on every request, like
    /// [`DataGrid::scope`]: `.scope_any(Product::color, p.colors.clone())` for the colors a facet
    /// panel chose. No values keep every row.
    pub fn scope_any<T: FieldType>(
        mut self,
        field: Field<E, T>,
        values: impl IntoIterator<Item = T>,
    ) -> Self {
        self.scope
            .extend(any_filter(field.meta(), values));
        self
    }

    /// Keeps only the rows whose `field` is between `from` and `to`, both inclusive, on every
    /// request, like [`DataGrid::scope`]; `None` leaves that end open.
    pub fn scope_range<T: FieldType>(
        mut self,
        field: Field<E, T>,
        from: Option<T>,
        to: Option<T>,
    ) -> Self {
        self.scope
            .extend(range_filter(field.meta(), from, to));
        self
    }

    /// Whether rows show the detail expander for this page's user.
    fn expandable(&self, r: &Render) -> bool {
        !matches!(self.paging, Paging::Virtual { .. })
            && !(self.nested() && self.tree.is_some())
            && self
                .detail
                .as_ref()
                .is_some_and(|d| r.can(&d.access))
    }

    /// Cells before the first data column: the detail expander and the selection box.
    fn lead(&self, r: &Render) -> usize {
        usize::from(self.expandable(r)) + usize::from(self.selectable)
    }

    /// The pinned column class of every cell position of a row (the leading cells, the shown
    /// columns, the action cell), empty for cells that scroll. Pinned cells are found by class,
    /// so the browser matches them with one lookup instead of `:nth-child` rules over every cell
    /// of every grid; `grid-columns.js` puts the offsets in `--z-ps1..5` and `--z-pe1..3`.
    fn pins(&self, r: &Render, cols: &[Shown]) -> Vec<&'static str> {
        const START: [&str; 5] = [
            " z-pin z-ps1",
            " z-pin z-ps2",
            " z-pin z-ps3",
            " z-pin z-ps4",
            " z-pin z-ps5",
        ];
        const START_EDGE: [&str; 5] = [
            " z-pin z-ps1 z-pin-edge-s",
            " z-pin z-ps2 z-pin-edge-s",
            " z-pin z-ps3 z-pin-edge-s",
            " z-pin z-ps4 z-pin-edge-s",
            " z-pin z-ps5 z-pin-edge-s",
        ];
        const END: [&str; 3] = [" z-pin z-pe1", " z-pin z-pe2", " z-pin z-pe3"];
        const END_EDGE: [&str; 3] = [
            " z-pin z-pe1 z-pin-edge-e",
            " z-pin z-pe2 z-pin-edge-e",
            " z-pin z-pe3 z-pin-edge-e",
        ];
        let lead = self.lead(r);
        let actions = usize::from(!self.row_actions.is_empty());
        let mut pins = vec![""; lead + cols.len() + actions];
        let starts = (lead
            + cols
                .iter()
                .take_while(|s| s.pin == Some(Pin::Start))
                .count())
        .min(5);
        let ends = (actions
            + cols
                .iter()
                .rev()
                .take_while(|s| s.pin == Some(Pin::End))
                .count())
        .min(3);
        for (k, class) in pins.iter_mut().take(starts).enumerate() {
            *class = if k + 1 == starts {
                START_EDGE[k]
            } else {
                START[k]
            };
        }
        let n = pins.len();
        for k in 0..ends.min(n - starts) {
            pins[n - 1 - k] = if k + 1 == ends { END_EDGE[k] } else { END[k] };
        }
        pins
    }

    /// Puts the grid's scope filters in front of the client's.
    fn scoped(&self, query: &mut DataQuery) {
        if !self.scope.is_empty() {
            query
                .filters
                .splice(0..0, self.scope.iter().cloned());
        }
    }

    /// Lets users export every row that matches their filters as CSV, in their column layout and
    /// language. The server pages through the grid's action and streams the file, up to
    /// [`DEFAULT_EXPORT_LIMIT`] rows. Exporting needs the grid's read access. The toolbar's
    /// `Export::Csv` button uses this export when the grid has it.
    pub fn exportable(mut self) -> Self {
        self.export = Some((self.action.access, DEFAULT_EXPORT_LIMIT));
        self
    }

    /// Like `exportable`, but exporting also needs `permission`.
    pub fn export_permission(mut self, permission: &'static str) -> Self {
        let limit = self
            .export
            .map_or(DEFAULT_EXPORT_LIMIT, |e| e.1);
        self.export = Some((crate::action::Access::Permission(permission), limit));
        self
    }

    /// Most rows one export may hold; bigger results are cut there. At most 1,000,000.
    pub fn export_limit(mut self, rows: usize) -> Self {
        let access = self
            .export
            .map_or(self.action.access, |e| e.0);
        self.export = Some((access, rows.clamp(1, 1_000_000)));
        self
    }

    /// Lets users edit cells of the columns marked `editable()` in place, saved with `save`, an
    /// action that takes `Valid<E>` like a form's. The server loads the row again by key, applies
    /// only the editable columns and the row version (`#[version]`) the client sends, validates
    /// and runs the action; a changed row comes back as a conflict. F2 or a double click on an
    /// editable cell starts editing, Enter saves, Escape cancels, Ctrl+Z undoes the last save.
    pub fn edit_with<A>(mut self, _save: A) -> Self
    where
        A: crate::form::FormAction<E> + Action<Input = Valid<E>>,
        A::Output: Outcome,
    {
        self.save = Some(SaveDef {
            meta: A::META,
            run: Box::new(|valid| runner::<A>(valid)),
        });
        self
    }

    /// Indexes in `E::fields()` of the columns users may edit: marked `editable()`, not keys, read
    /// only fields or row versions.
    fn editable_fields(&self) -> Vec<usize> {
        self.columns
            .iter()
            .filter(|c| c.editable && !c.meta.key && !c.meta.readonly && !c.meta.version)
            .filter_map(|c| {
                E::fields()
                    .iter()
                    .position(|f| f.name == c.meta.name)
            })
            .collect()
    }

    /// Whether the page's user may save inline edits.
    fn can_edit(&self, r: &Render) -> bool {
        self.save
            .as_ref()
            .is_some_and(|s| r.can(&s.meta.access))
    }

    /// Context menu of each row, built on the server from the row. Right click, Shift+F10 or the
    /// context menu key opens it. See [`crate::ContextMenu`].
    pub fn row_menu(
        mut self,
        menu: impl Fn(&E) -> crate::menu::ContextMenu + Send + Sync + 'static,
    ) -> Self {
        self.row_menu = Some(Box::new(menu));
        self
    }

    /// DataGrid with a column for every field of the model except the row version, in declaration
    /// order, and filter boxes on text and choice columns. The quickest list for a simple model.
    pub fn auto<A>(source: A) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<Page<E>>>,
    {
        let mut grid = Self::of(source);
        for (i, meta) in E::fields()
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.version)
        {
            let filter = matches!(
                meta.kind,
                crate::value::FieldKind::Text | crate::value::FieldKind::Choice(_)
            );
            grid.columns.push(Column {
                value: Box::new(move |e: &E| {
                    std::borrow::Cow::Owned(e.value_at(i).unwrap_or(Value::Null))
                }),
                filter,
                ..Column::of(meta)
            });
        }
        grid
    }

    /// Shades every other row, for very wide grids where the eye loses the row. Rows are
    /// divided by lines only unless set.
    pub fn zebra(mut self) -> Self {
        self.zebra = true;
        self
    }

    pub fn mode(mut self, mode: DataMode) -> Self {
        self.mode = mode;
        self
    }

    /// Paging style. Numbered pages of 50 rows unless set.
    pub fn paging(mut self, paging: Paging) -> Self {
        self.paging = paging;
        self
    }

    /// Adds a column. The following settings (`width`, `filter`...) apply to it.
    pub fn column<T: FieldType + 'static>(mut self, field: Field<E, T>) -> DataGridColumn<E, T> {
        self.columns.push(Column {
            value: Box::new(move |e| std::borrow::Cow::Owned(field.get(e).to_value())),
            ..Column::of(field.meta())
        });
        DataGridColumn {
            grid: self,
            field,
            _type: PhantomData,
        }
    }

    /// Picks a highlight per row.
    pub fn row_style(
        mut self,
        style: impl Fn(&E) -> Option<RowStyle> + Send + Sync + 'static,
    ) -> Self {
        self.row_style = Some(Box::new(style));
        self
    }

    /// What happens when a row is opened (double click or Enter).
    pub fn on_open(mut self, open: impl Fn(&E) -> Effect + Send + Sync + 'static) -> Self {
        self.on_open = Some(Box::new(open));
        self
    }

    /// Adds a button to every row that runs `action` with input built from the row:
    /// `.row_action(t!("stock.archive"), archive_product, |p| p.id)`. The button only shows for
    /// users with the action's permission. When pressed, the server loads the row again by its key
    /// through the grid's action, builds the input from it, checks the permission again and runs
    /// the action; the client never sends the input.
    pub fn row_action<A>(
        mut self,
        label: impl Into<Text>,
        _action: A,
        input: impl Fn(&E) -> A::Input + Send + Sync + 'static,
    ) -> Self
    where
        A: Action,
        A::Output: Outcome,
    {
        self.row_actions.push(RowAction {
            label: label.into(),
            icon: None,
            meta: A::META,
            run: Box::new(move |row| runner::<A>(input(row))),
            ask: Ask::default(),
        });
        self.last_bulk = false;
        self
    }

    /// Adds a checkbox to every row and a selection bar above the grid. Shift selects a range,
    /// the header box selects the page, and the bar offers every row that matches the filters.
    pub fn selectable(mut self) -> Self {
        self.selectable = true;
        self
    }

    /// Adds a button to the selection bar that runs `action` with the keys of the selected rows:
    /// `.bulk_action(t!("stock.archive"), archive_products, Product::id)` where `archive_products`
    /// takes `Vec<ProductId>`. Makes the grid selectable. The button shows only for users with the
    /// action's permission, which the server checks again. When the user picks every matching
    /// row, the server collects the keys itself (at most [`MAX_BULK`]). Keys picked one by one come
    /// from the client, so the action must check that the user may change each record, as an API
    /// action would.
    pub fn bulk_action<A, K>(
        mut self,
        label: impl Into<Text>,
        _action: A,
        _key: Field<E, K>,
    ) -> Self
    where
        A: BulkAction<K> + Action<Input = Vec<K>>,
        A::Output: Outcome,
        K: serde::de::DeserializeOwned + Send + 'static,
    {
        self.selectable = true;
        self.bulk.push(BulkDef {
            label: label.into(),
            meta: A::META,
            run: Box::new(|keys| {
                let keys: Vec<K> = keys
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<std::result::Result<_, _>>()
                    .ok()?;
                Some(runner::<A>(keys))
            }),
            ask: Ask::default(),
        });
        self.last_bulk = true;
        self
    }

    /// The ask of the last added row or bulk action.
    fn last_ask(&mut self) -> Option<&mut Ask> {
        if self.last_bulk {
            self.bulk.last_mut().map(|b| &mut b.ask)
        } else {
            self.row_actions
                .last_mut()
                .map(|a| &mut a.ask)
        }
    }

    /// Asks the user to confirm with this message before the last added row or bulk action
    /// runs, in the same dialog as [`crate::ui::ActionButton::confirm`]. Cancel has the focus
    /// first. The question is asked in the browser; an action that must never run without a
    /// person also declares `confirm` for AI agents.
    ///
    /// ```ignore
    /// DataGrid::of(search_customers)
    ///     .bulk_action(t!("customers.delete"), delete_customers, Customer::id)
    ///     .confirm(t!("customers.delete_confirm"))
    ///     .tone(Tone::Danger)
    /// ```
    pub fn confirm(mut self, message: impl Into<Text>) -> Self {
        if let Some(a) = self.last_ask() {
            a.confirm = Some(message.into());
        }
        self
    }

    /// `Tone::Danger` draws the last added row or bulk action, and its confirmation, in red for
    /// actions that remove or cancel things. Other tones keep the normal look.
    pub fn tone(mut self, tone: Tone) -> Self {
        if let Some(a) = self.last_ask() {
            a.danger = tone == Tone::Danger;
        }
        self
    }

    /// Icon of the last added row action.
    pub fn icon(mut self, icon: Icon) -> Self {
        if let Some(a) = self.row_actions.last_mut() {
            a.icon = Some(icon);
        }
        self
    }

    /// Index of the `#[key]` field, if the model has one.
    fn key_index() -> Option<usize> {
        E::fields().iter().position(|f| f.key)
    }

    /// Effect produced when the row is opened.
    pub fn open_effect(&self, row: &E) -> Option<Effect> {
        self.on_open.as_ref().map(|f| f(row))
    }

    /// Renders rows as HTML in English. For tests and tools; the server uses the user's language.
    pub fn rows_html(&self, rows: &[E]) -> String {
        self.rows(
            rows,
            &Render::english("grid"),
            &self.layout(None),
            &Branch::default(),
        )
    }

    /// The view's columns that exist, in its order, without repeats; may be empty.
    fn view_columns(&self, v: &DataGridView) -> Vec<Shown> {
        let mut out: Vec<Shown> = Vec::new();
        for cv in v
            .columns
            .iter()
            .take(self.columns.len())
        {
            let Some(index) = self
                .columns
                .iter()
                .position(|c| c.name == cv.field)
            else {
                continue;
            };
            if out.iter().any(|s| s.index == index) {
                continue;
            }
            let width = cv
                .width
                .map(|w| w.clamp(COLUMN_WIDTH.0, COLUMN_WIDTH.1))
                .or(self.columns[index].width);
            out.push(Shown {
                index,
                width,
                pin: cv.pin,
            });
        }
        out
    }

    /// Shown columns for a view: the view's columns that exist, in its order, with starts pinned
    /// first and ends last; or the grid's own columns without the hidden ones.
    fn layout(&self, view: Option<&DataGridView>) -> Vec<Shown> {
        let from_view = self
            .view_for(view)
            .filter(|v| !v.columns.is_empty())
            .map(|v| self.view_columns(v))
            .filter(|cols| !cols.is_empty());
        let mut shown: Vec<Shown> = match from_view {
            Some(cols) => cols,
            None => self
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.hidden)
                .map(|(index, c)| Shown {
                    index,
                    width: c.width,
                    pin: c.pin,
                })
                .collect(),
        };
        for side in [Pin::Start, Pin::End] {
            let mut n = 0;
            for s in shown
                .iter_mut()
                .filter(|s| s.pin == Some(side))
            {
                n += 1;
                if n > MAX_PINNED {
                    s.pin = None;
                }
            }
        }
        let rank = |s: &Shown| match s.pin {
            Some(Pin::Start) => 0,
            None => 1,
            Some(Pin::End) => 2,
        };
        shown.sort_by_key(rank);
        shown
    }

    /// Cleans a view from the client: unknown fields, repeats and out of range widths go, sort and
    /// filters keep only what the grid allows and the limits.
    fn clean(&self, view: DataGridView) -> DataGridView {
        let columns = self
            .layout(Some(&view))
            .into_iter()
            .map(|s| ColumnView {
                field: self.columns[s.index]
                    .meta
                    .name
                    .to_string(),
                width: s.width,
                pin: s.pin,
            })
            .collect();
        let mut query = DataQuery {
            sort: view.sort,
            filters: view.filters,
            ..DataQuery::default()
        };
        self.allow(&mut query, None);
        query.filters.retain(|f| {
            let short = |s: &Option<String>| {
                s.as_deref()
                    .is_none_or(|t| t.chars().count() <= MAX_FILTER_TEXT)
            };
            f.contains.chars().count() <= MAX_FILTER_TEXT
                && short(&f.starts_with)
                && short(&f.equals)
                && short(&f.from)
                && short(&f.to)
                && f.any_of.len() <= MAX_FILTER_CHOICES
                && f.any_of
                    .iter()
                    .all(|v| v.chars().count() <= MAX_FILTER_TEXT)
        });
        DataGridView {
            columns,
            sort: query.sort,
            filters: query.filters,
            shape: None,
        }
    }

    /// The view when it fits the grid: always for typed grids, and for a dynamic grid only when
    /// it was made for the same columns, so new columns are never hidden by an old view.
    fn view_for<'v>(&self, view: Option<&'v DataGridView>) -> Option<&'v DataGridView> {
        view.filter(|v| self.shape.is_none() || v.shape == self.shape)
    }

    /// Keeps the totals, filters and sort keys this grid offers, within the limits. With a ctx,
    /// filters and sorts on masked fields the user may not reveal go too, so every read of the
    /// grid (pages, exports, bulk runs, saved views, agents) is cleaned the same way.
    fn allow(&self, query: &mut DataQuery, ctx: Option<&Ctx>) {
        if let Some(ctx) = ctx {
            crate::privacy::strip_masked(query, E::fields(), ctx);
        }
        let allowed = self.aggregate_requests();
        query
            .aggregates
            .retain(|a| allowed.contains(a));
        query.aggregates.dedup();
        let sortable = |name: &str| {
            self.columns
                .iter()
                .any(|c| c.sort && c.name == name)
        };
        let filterable = |name: &str| {
            self.columns
                .iter()
                .any(|c| c.filter && c.name == name)
        };
        query
            .filters
            .retain(|f| filterable(&f.field) && !f.is_empty());
        query.filters.truncate(MAX_FILTERS);
        let mut seen = Vec::new();
        query.sort.retain(|s| {
            sortable(&s.field) && !seen.contains(&s.field) && {
                seen.push(s.field.clone());
                true
            }
        });
        query.sort.truncate(MAX_SORT);
    }

    /// Header row for the shown columns.
    fn head_row(&self, r: &Render, cols: &[Shown]) -> String {
        let pins = self.pins(r, cols);
        let lead = self.lead(r);
        let mut out = String::from("<tr>");
        if self.expandable(r) {
            out.push_str(&format!(
                r#"<th scope="col" class="z-expand-col{}" data-pin="start"><span class="z-sr-only">{}</span></th>"#,
                pins[0],
                r.text(&Text::key("zeyn.grid.details"))
            ));
        }
        if self.selectable {
            out.push_str(&format!(
                r#"<th scope="col" class="z-select-col{}" data-pin="start"><input type="checkbox" data-zeyn-select-all aria-label="{}"></th>"#,
                pins[lead - 1],
                r.text(&Text::key("zeyn.grid.select_page"))
            ));
        }
        for (pos, s) in cols.iter().enumerate() {
            let c = &self.columns[s.index];
            let mut style = String::new();
            if let Some(w) = s.width {
                style.push_str(&format!("width:{w}px;"));
            }
            if let Some(f) = c.flex.filter(|_| s.width.is_none()) {
                style.push_str(&format!("--z-flex:{f};"));
            }
            let num = matches!(
                c.meta.kind,
                FieldKind::Integer | FieldKind::Money | FieldKind::Decimal
            );
            let pin = pins[lead + pos];
            let class = match (num, pin.is_empty()) {
                (true, true) => r#" class="z-num""#.to_string(),
                (true, false) => format!(r#" class="z-num{pin}""#),
                (false, true) => String::new(),
                (false, false) => format!(r#" class="{}""#, pin.trim_start()),
            };
            let sort = if c.sort {
                r#" aria-sort="none""#
            } else {
                r#" data-sort="off""#
            };
            out.push_str(&format!(
                r#"<th scope="col"{class} data-field="{}"{sort} aria-colindex="{}""#,
                escape(&c.name),
                pos + 1 + self.lead(r)
            ));
            if !style.is_empty() {
                out.push_str(&format!(r#" style="{style}""#));
            }
            if let Some(p) = s.pin {
                out.push_str(&format!(r#" data-pin="{}""#, p.code()));
            }
            if self.can_edit(r) && c.editable && !c.meta.readonly && !c.meta.key && !c.meta.version
            {
                out.push_str(&format!(r#" data-edit="{}""#, filter_kind(&c.meta.kind)));
                if c.meta.optional {
                    out.push_str(r#" data-optional="true""#);
                }
                if let (FieldKind::Choice(options), false) = (c.meta.kind, c.filter) {
                    let list: Vec<serde_json::Value> = options
                        .iter()
                        .map(|o| json!({ "value": o.value, "label": r.i18n.text(r.locale, &Text::key_or(o.key, o.label)) }))
                        .collect();
                    out.push_str(&format!(r#" data-options="{}""#, attr(&list)));
                }
            }
            if c.filter {
                out.push_str(&format!(r#" data-filter="{}""#, filter_kind(&c.meta.kind)));
                if let FieldKind::Choice(options) = c.meta.kind {
                    let list: Vec<serde_json::Value> = options
                        .iter()
                        .map(|o| json!({ "value": o.value, "label": r.i18n.text(r.locale, &Text::key_or(o.key, o.label)) }))
                        .collect();
                    out.push_str(&format!(r#" data-options="{}""#, attr(&list)));
                }
            }
            match &c.label {
                Some(label) => {
                    out.push('>');
                    escape_into(&mut out, label);
                    out.push_str("</th>");
                }
                None => out.push_str(&format!(">{}</th>", r.label(c.meta))),
            }
        }
        if !self.row_actions.is_empty() {
            out.push_str(&format!(
                r#"<th scope="col" class="z-actions-col{}" data-pin="end" aria-colindex="{}"><span class="z-sr-only">{}</span></th>"#,
                pins[pins.len() - 1],
                cols.len() + 1 + self.lead(r),
                r.text(&Text::key("zeyn.grid.actions"))
            ));
        }
        out.push_str("</tr>");
        out
    }

    /// The `<tr>` of each row. Work that is the same for every row (access checks, labels, the
    /// row action cell, which columns are editable) is done once, and cells are written straight
    /// into the output without a string per cell.
    fn rows(&self, rows: &[E], r: &Render, cols: &[Shown], b: &Branch) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(rows.len() * (cols.len() + 1) * 32);
        let nested = self.nested();
        let tree = nested && self.tree.is_some();
        let edit = self.can_edit(r);
        let key_at = Self::key_index();
        let version_at = edit
            .then(|| {
                E::fields()
                    .iter()
                    .position(|f| f.version)
            })
            .flatten();
        let detail = self
            .detail
            .as_ref()
            .filter(|_| self.expandable(r));
        let pins = self.pins(r, cols);
        let lead = self.lead(r);
        let expand_cell = detail.map(|_| {
            format!(
                r#"<td class="z-expand{}"><button type="button" class="z-expander" data-zeyn-expand aria-expanded="false" aria-label="{}" tabindex="-1"></button></td>"#,
                pins[0],
                r.text(&Text::key("zeyn.grid.details"))
            )
        });
        let select_cell = self.selectable.then(|| {
            format!(
                r#"<td class="z-row-select{}"><input type="checkbox" data-zeyn-select tabindex="-1" aria-label="{}"></td>"#,
                pins[lead - 1],
                r.text(&Text::key("zeyn.grid.select_row"))
            )
        });
        let tree_toggle = tree.then(|| {
            format!(
                r#"<button type="button" class="z-expander" data-zeyn-expand aria-expanded="false" aria-label="{}" tabindex="-1"></button>"#,
                r.text(&Text::key("zeyn.grid.children"))
            )
        });
        let actions_cell = (!self.row_actions.is_empty()).then(|| {
            let mut cell = format!(r#"<td class="z-row-actions{}">"#, pins[pins.len() - 1]);
            for (i, a) in self.row_actions.iter().enumerate().filter(|(_, a)| r.can(&a.meta.access)) {
                let label = r.text(&a.label);
                let icon = a.icon.map(Icon::html).unwrap_or_default();
                let class = if a.ask.danger {
                    "z-row-action z-danger"
                } else {
                    "z-row-action"
                };
                let _ = write!(
                    cell,
                    r#"<button type="button" class="{class}" data-zeyn-run="a{i}"{} tabindex="-1">{icon}<span>{label}</span></button>"#,
                    a.ask.attr(r)
                );
            }
            cell.push_str("</td>");
            cell
        });
        let editable: Vec<bool> = cols
            .iter()
            .map(|s| {
                let c = &self.columns[s.index];
                edit && c.editable
                    && !c.meta.readonly
                    && !c.meta.key
                    && !c.meta.version
                    && c.meta.mask.is_none()
            })
            .collect();
        for row in rows {
            if let (true, Some(ctx)) = (E::SENSITIVE, r.ctx) {
                ctx.record_read(row);
            }
            out.push_str("<tr");
            let mut toggle = "";
            if nested {
                let _ = write!(
                    out,
                    r#" aria-level="{}" data-zeyn-level="{}""#,
                    b.depth + 1,
                    b.depth
                );
            }
            if tree {
                let key = key_at
                    .and_then(|i| row.value_at(i))
                    .and_then(|v| value_key(&v));
                toggle = match key.filter(|k| b.parents.contains(k) && b.depth + 1 < MAX_TREE_DEPTH)
                {
                    Some(k) => {
                        let mut path = b.path.to_vec();
                        path.push(Some(k));
                        let _ = write!(
                            out,
                            r#" aria-expanded="false" data-zeyn-path="{}""#,
                            attr(&path)
                        );
                        tree_toggle
                            .as_deref()
                            .unwrap_or_default()
                    }
                    None => r#"<span class="z-tree-spacer" aria-hidden="true"></span>"#,
                };
            }
            if let Some(style) = self
                .row_style
                .as_ref()
                .and_then(|f| f(row))
            {
                let _ = write!(out, r#" class="{}""#, style.css());
            }
            if let Some(effect) = self.open_effect(row) {
                let _ = write!(
                    out,
                    r#" tabindex="-1" data-zeyn-effect="{}""#,
                    attr(&effect)
                );
            }
            if let Some(k) = key_at.and_then(|i| row.value_at(i)) {
                out.push_str(r#" data-zeyn-key=""#);
                key_into(&mut out, &k);
                out.push('"');
            }
            if let Some(v) = version_at.and_then(|i| row.value_at(i)) {
                out.push_str(r#" data-zeyn-version=""#);
                key_into(&mut out, &v);
                out.push('"');
            }
            if let Some(menu) = self.row_menu.as_ref().map(|f| f(row)) {
                let items = menu.to_json(r);
                if items
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
                {
                    let _ = write!(out, r#" data-zeyn-menu="{}""#, attr(&items));
                }
            }
            if let Some(d) = detail {
                let target = json!({ "screen": d.screen, "params": (d.params)(row) });
                let _ = write!(
                    out,
                    r#" aria-expanded="false" data-zeyn-detail="{}""#,
                    attr(&target)
                );
            }
            out.push('>');
            out.push_str(
                expand_cell
                    .as_deref()
                    .unwrap_or_default(),
            );
            out.push_str(
                select_cell
                    .as_deref()
                    .unwrap_or_default(),
            );
            for (n, s) in cols.iter().enumerate() {
                let c = &self.columns[s.index];
                let value = (c.value)(row);
                let num = matches!(
                    *value,
                    Value::Int(_) | Value::Money(_) | Value::Decimal(_) | Value::Number(_)
                );
                let first = tree && n == 0;
                out.push_str("<td");
                let pin = pins[lead + n];
                if num || first || c.wrap || !pin.is_empty() {
                    out.push_str(r#" class=""#);
                    let mut sep = "";
                    for (on, class) in [(num, "z-num"), (first, "z-tree-cell"), (c.wrap, "z-wrap")]
                    {
                        if on {
                            out.push_str(sep);
                            out.push_str(class);
                            sep = " ";
                        }
                    }
                    out.push_str(if sep.is_empty() {
                        pin.trim_start()
                    } else {
                        pin
                    });
                    out.push('"');
                }
                if editable[n] {
                    out.push_str(r#" data-v=""#);
                    escape_into(&mut out, &crate::form::edit_value(&value, r.locale, r.tz));
                    out.push('"');
                }
                out.push('>');
                if first {
                    let _ = write!(
                        out,
                        r#"<span class="z-tree" style="--z-level:{}">{toggle}<span>"#,
                        b.depth
                    );
                }
                match (c.meta.mask, &c.cell) {
                    (Some(_), _) => out.push_str(&crate::privacy::cell(r, c.meta, &value, row)),
                    (None, Some(cell)) => out.push_str(&cell(row, r)),
                    (None, None) => match *value {
                        Value::Int(n) if c.plain => {
                            let _ = write!(out, "{n}");
                        }
                        _ => r.value_into(&mut out, &value, &c.meta.kind),
                    },
                }
                if first {
                    out.push_str("</span></span>");
                }
                out.push_str("</td>");
            }
            out.push_str(
                actions_cell
                    .as_deref()
                    .unwrap_or_default(),
            );
            out.push_str("</tr>");
        }
        out
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let scope = r.scope;
        let view = self.view_for(page.and_then(|p| p.view.as_ref()));
        let cols = self.layout(view);
        if self.selectable {
            let t = |k: &str| r.text(&Text::key(format!("zeyn.grid.{k}")));
            out.push_str(&format!(
                r#"<div class="z-bulk" data-zeyn-bulk data-testid="{scope}.grid.selection" data-text-selected="{}" data-text-all="{}" data-text-all-selected="{}" hidden><span class="z-bulk-count" aria-live="polite"></span><button type="button" class="z-link" data-zeyn-select-matching hidden></button>"#,
                t("selected_count"),
                t("select_matching"),
                t("all_selected"),
            ));
            for (i, b) in self
                .bulk
                .iter()
                .enumerate()
                .filter(|(_, b)| r.can(&b.meta.access))
            {
                let class = if b.ask.danger {
                    "z-button z-danger-button"
                } else {
                    "z-button"
                };
                out.push_str(&format!(
                    r#"<button type="button" class="{class}" data-zeyn-bulk-run="{i}" data-testid="{scope}.grid.bulk-{i}"{}>{}</button>"#,
                    b.ask.attr(r),
                    r.text(&b.label)
                ));
            }
            out.push_str(&format!(
                r#"<button type="button" class="z-button" data-zeyn-select-clear>{}</button></div>"#,
                t("clear_selection")
            ));
        }
        let grid_text = |key: &str| r.text(&Text::key(format!("zeyn.grid.{key}")));
        out.push_str(&format!(
            r#"<div class="z-grid{}" data-zeyn-grid data-testid="{scope}.grid" data-zeyn-action="{}" data-zeyn-mode="{}" data-zeyn-paging="{}" data-zeyn-page-size="{}" data-zeyn-openable="{}" data-zeyn-text-range="{}" data-zeyn-text-range-total="{}" data-zeyn-text-loaded="{}""#,
            if self.zebra { " z-zebra" } else { "" },
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
            let row_height = match (row_height, r.ctx.is_some_and(Ctx::compact)) {
                (0, true) => 32,
                (0, false) => 40,
                (h, _) => h,
            };
            out.push_str(&format!(
                r#" data-zeyn-row-height="{row_height}" style="--z-row-height:{row_height}px""#
            ));
        }
        if let Some(p) = page {
            out.push_str(&format!(
                r#" data-zeyn-offset="{}" data-zeyn-count="{}""#,
                p.offset, p.count
            ));
            if let Some(t) = p.total {
                out.push_str(&format!(r#" data-zeyn-total="{t}""#));
            }
            if let Some(n) = &p.next {
                out.push_str(&format!(r#" data-zeyn-next="{}""#, escape(&n.0)));
            }
        }
        out.push_str(&format!(r#" data-zeyn-texts="{}""#, attr(&grid_texts(r))));
        out.push_str(&format!(
            r#" data-zeyn-columns="{}""#,
            attr(&self.columns_json(r))
        ));
        if let Some(shape) = &self.shape {
            out.push_str(&format!(r#" data-zeyn-shape="{}""#, escape(shape)));
        }
        if let Some(v) = view {
            out.push_str(&format!(r#" data-zeyn-view="{}""#, attr(v)));
        }
        if !self.default_sort.is_empty() {
            out.push_str(&format!(
                r#" data-zeyn-sort-default="{}""#,
                attr(&self.default_sort)
            ));
        }
        if page.is_some_and(|p| p.saved_views) {
            out.push_str(r#" data-zeyn-views="server""#);
        }
        if self.export.is_some_and(|e| r.can(&e.0)) {
            out.push_str(match (cfg!(feature = "xlsx"), cfg!(feature = "pdf")) {
                (true, true) => r#" data-zeyn-exportable="csv xlsx pdf""#,
                (true, false) => r#" data-zeyn-exportable="csv xlsx""#,
                (false, true) => r#" data-zeyn-exportable="csv pdf""#,
                (false, false) => r#" data-zeyn-exportable="csv""#,
            });
        }
        if self.can_edit(r)
            && let Some(v) = E::fields().iter().find(|f| f.version)
        {
            out.push_str(&format!(r#" data-zeyn-version-field="{}""#, v.name));
        }
        out.push_str(&format!(
            r#"><table role="{}" aria-colcount="{}"><thead>{}</thead><tbody data-zeyn-rows>"#,
            if self.nested() { "treegrid" } else { "grid" },
            self.columns.len() + usize::from(!self.row_actions.is_empty()) + self.lead(r),
            self.head_row(r, &cols)
        ));
        let mut later = String::new();
        let shown = self.lead(r) + cols.len() + usize::from(!self.row_actions.is_empty());
        let live_rows = if shown >= VIRTUAL_WIDE_COLUMNS {
            0
        } else {
            VIRTUAL_LIVE_ROWS
        };
        if let Some(p) = page {
            match (self.paging, Self::items(p)) {
                (Paging::Virtual { .. }, Some(rows))
                    if p.groups.is_none() && rows.len() > live_rows =>
                {
                    let branch = Branch {
                        depth: p.depth,
                        path: &p.path,
                        parents: &p.parents,
                    };
                    out.push_str(&self.rows(&rows[..live_rows], r, &cols, &branch));
                    later = self.rows(&rows[live_rows..], r, &cols, &branch);
                }
                _ => out.push_str(&self.rows_html_erased(p, r)),
            }
        }
        out.push_str("</tbody>");
        if self
            .columns
            .iter()
            .any(|c| !c.aggregates.is_empty())
        {
            out.push_str("<tfoot data-zeyn-foot>");
            out.push_str(
                &self.foot_row(
                    r,
                    page.map(|p| p.totals.as_slice())
                        .unwrap_or_default(),
                    &cols,
                ),
            );
            out.push_str("</tfoot>");
        }
        out.push_str("</table>");
        if !later.is_empty() {
            out.push_str("<template data-zeyn-rows-later>");
            out.push_str(&later);
            out.push_str("</template>");
        }
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

    /// Every column, shown or not, for the column chooser: `[{field, label, hidden, pin, width}]`.
    fn columns_json(&self, r: &Render) -> Vec<serde_json::Value> {
        self.columns
            .iter()
            .map(|c| json!({ "field": c.name, "label": c.label_plain(r), "hidden": c.hidden, "pin": c.pin, "width": c.width }))
            .collect()
    }

    /// Footer row with one cell per column. Cells of columns with totals carry `data-field` and
    /// `data-agg` (space separated ops) and show each total with its localized name.
    fn foot_row(&self, r: &Render, totals: &[Total], cols: &[Shown]) -> String {
        let pins = self.pins(r, cols);
        let lead = self.lead(r);
        let empty = |pin: &str| {
            if pin.is_empty() {
                "<td></td>".to_string()
            } else {
                format!(r#"<td class="{}"></td>"#, pin.trim_start())
            }
        };
        let mut out = String::from("<tr>");
        for pin in &pins[..lead] {
            out.push_str(&empty(pin));
        }
        for (n, c) in cols
            .iter()
            .map(|s| &self.columns[s.index])
            .enumerate()
        {
            let cell = self.total_cell(c, totals, r);
            let pin = pins[lead + n];
            if pin.is_empty() {
                out.push_str(&cell);
            } else if let Some(rest) = cell.strip_prefix(r#"<td class=""#) {
                out.push_str(&format!(
                    r#"<td class="{}{rest}"#,
                    pin.trim_start().to_string() + " "
                ));
            } else {
                out.push_str(&format!(
                    r#"<td class="{}"{}"#,
                    pin.trim_start(),
                    &cell[3..]
                ));
            }
        }
        if !self.row_actions.is_empty() {
            out.push_str(&empty(pins[pins.len() - 1]));
        }
        out.push_str("</tr>");
        out
    }

    /// The cell of one column with its totals, or an empty cell.
    fn total_cell(&self, c: &Column<E>, totals: &[Total], r: &Render) -> String {
        if c.aggregates.is_empty() {
            return "<td></td>".to_string();
        }
        let ops: Vec<&str> = c
            .aggregates
            .iter()
            .map(|a| a.code())
            .collect();
        let num = matches!(
            c.meta.kind,
            FieldKind::Integer | FieldKind::Money | FieldKind::Decimal
        );
        let class = if num { r#" class="z-num""# } else { "" };
        let mut out = format!(
            r#"<td{class} data-field="{}" data-agg="{}">"#,
            c.meta.name,
            ops.join(" ")
        );
        for op in &c.aggregates {
            let value = totals
                .iter()
                .find(|t| t.field == c.meta.name && t.op == *op)
                .map(|t| match t.value {
                    Value::Int(n) => grouped(n, r.locale),
                    ref v => r.value(v, &c.meta.kind),
                })
                .unwrap_or_default();
            out.push_str(&format!(
                r#"<span class="z-agg" data-op="{}"><span class="z-agg-label">{}</span> <span class="z-agg-value">{value}</span></span>"#,
                op.code(),
                r.text(&Text::key(format!("zeyn.grid.agg.{}", op.code()))),
            ));
        }
        out.push_str("</td>");
        out
    }

    /// Header rows of groups at `depth`: the expander, column label, value and row count span
    /// the columns up to the first one with totals; the totals of each group follow under their
    /// columns.
    fn group_rows(
        &self,
        groups: &[crate::action::Group],
        r: &Render,
        cols: &[Shown],
        depth: usize,
        path: &[Option<String>],
    ) -> String {
        let Some(meta) = self.group_by.get(depth) else {
            return String::new();
        };
        let lead = self.lead(r);
        let first = cols
            .iter()
            .position(|s| {
                !self.columns[s.index]
                    .aggregates
                    .is_empty()
            })
            .unwrap_or(cols.len());
        let mut out = String::new();
        for g in groups {
            let mut here = path.to_vec();
            here.push(value_key(&g.value));
            let shown = match g.value {
                Value::Null => r.text(&Text::key("zeyn.grid.blank")),
                ref v => plain_html(v, &meta.kind, r),
            };
            let label = r.label(meta);
            let count = r.text(
                &Text::key("zeyn.grid.group_count").arg("n", grouped(g.count as i64, r.locale)),
            );
            out.push_str(&format!(
                r#"<tr class="z-group" aria-level="{level}" data-zeyn-level="{depth}" aria-expanded="false" data-zeyn-path="{}"><td colspan="{}"><span class="z-group-head" style="--z-level:{depth}"><button type="button" class="z-expander" data-zeyn-expand aria-expanded="false" aria-label="{label}: {shown}" tabindex="-1"></button><span class="z-group-label">{label}: <strong>{shown}</strong></span> <span class="z-group-count">{count}</span></span></td>"#,
                attr(&here),
                (lead + first).max(1),
                level = depth + 1,
            ));
            for s in &cols[first..] {
                out.push_str(&self.total_cell(&self.columns[s.index], &g.totals, r));
            }
            if !self.row_actions.is_empty() {
                out.push_str("<td></td>");
            }
            out.push_str("</tr>");
        }
        out
    }

    /// Totals the columns declare, as a query asks for them.
    fn aggregate_requests(&self) -> Vec<AggregateRequest> {
        self.columns
            .iter()
            .flat_map(|c| {
                c.aggregates
                    .iter()
                    .map(|op| AggregateRequest {
                        field: c.meta.name.to_string(),
                        op: *op,
                    })
            })
            .collect()
    }

    fn rows_html_erased(&self, page: &LoadedPage, r: &Render) -> String {
        let cols = self.layout(page.view.as_ref());
        if let Some(groups) = &page.groups {
            return self.group_rows(groups, r, &cols, page.depth, &page.path);
        }
        let branch = Branch {
            depth: page.depth,
            path: &page.path,
            parents: &page.parents,
        };
        Self::items(page)
            .map(|rows| self.rows(rows, r, &cols, &branch))
            .unwrap_or_default()
    }

    /// The rows of a loaded page: the action's own, or the rows of a dynamic page.
    fn items(page: &LoadedPage) -> Option<&[E]> {
        match page.items.downcast_ref::<Vec<E>>() {
            Some(rows) => Some(rows),
            None => {
                let rows: &dyn std::any::Any = &page
                    .items
                    .downcast_ref::<dynamic::DynamicItems>()?
                    .rows;
                rows.downcast_ref::<Vec<E>>()
                    .map(Vec::as_slice)
            }
        }
    }

    /// Loads the groups or rows under `path` of a grouped or tree grid. Group values and parent
    /// keys become filters in front of the client's; a tree also asks which of the loaded rows
    /// have children (one grouped query on the parent field) and, at the top, the totals over
    /// every matching row.
    fn fetch_nested(
        &self,
        ctx: Ctx,
        mut query: DataQuery,
        path: Vec<Option<String>>,
    ) -> BoxFuture<Result<LoadedPage>> {
        use crate::action::Error;
        self.allow(&mut query, Some(&ctx));
        query.time_zone = ctx.time_zone();
        let depth = path.len();
        let mut fixed = self.scope.clone();
        let user = query.filters.clone();
        match self.tree {
            Some(parent) => {
                if depth >= MAX_TREE_DEPTH || path.iter().any(Option::is_none) {
                    return Box::pin(async { Err(Error::NotFound) });
                }
                fixed.push(key_filter(
                    parent,
                    path.last()
                        .cloned()
                        .flatten()
                        .as_deref(),
                ));
            }
            None => {
                if depth > self.group_by.len() {
                    return Box::pin(async { Err(Error::NotFound) });
                }
                for (meta, key) in self.group_by.iter().zip(&path) {
                    fixed.push(key_filter(meta, key.as_deref()));
                }
                if let Some(meta) = self.group_by.get(depth) {
                    query.group = Some(meta.name.to_string());
                    query.aggregates = self.aggregate_requests();
                }
            }
        }
        query.filters.splice(0..0, fixed);
        let offset = match &query.page {
            PageRequest::Offset { offset, .. } => *offset,
            PageRequest::After { .. } => 0,
        };
        let fetch = self.fetch;
        let parent = self.tree;
        let scope = self.scope.clone();
        Box::pin(async move {
            let grouped = query.group.is_some();
            let totals_query = (parent.is_some() && depth == 0 && !query.aggregates.is_empty())
                .then(|| DataQuery {
                    page: PageRequest::Offset {
                        offset: 0,
                        limit: 1,
                    },
                    filters: scope
                        .iter()
                        .cloned()
                        .chain(user.iter().cloned())
                        .collect(),
                    aggregates: query.aggregates.clone(),
                    time_zone: query.time_zone,
                    ..DataQuery::default()
                });
            let page = fetch(ctx.clone(), query.clone()).await?;
            let mut parents = Vec::new();
            if let (Some(parent), Some(ki)) = (parent, Self::key_index()) {
                let keys: Vec<String> = page
                    .items
                    .iter()
                    .filter_map(|row| row.value_at(ki))
                    .filter_map(|v| value_key(&v))
                    .collect();
                if !keys.is_empty() {
                    let mut filters = scope.clone();
                    filters.extend(user.iter().cloned());
                    filters.push(Filter::any_of(parent.name, keys));
                    let ask = DataQuery {
                        page: PageRequest::Offset {
                            offset: 0,
                            limit: MAX_PAGE_SIZE,
                        },
                        filters,
                        group: Some(parent.name.to_string()),
                        time_zone: query.time_zone,
                        ..DataQuery::default()
                    };
                    parents = fetch(ctx.clone(), ask)
                        .await?
                        .groups
                        .iter()
                        .filter_map(|g| value_key(&g.value))
                        .collect();
                }
            }
            let totals = match totals_query {
                Some(q) => fetch(ctx, q).await?.totals,
                None => page.totals,
            };
            Ok(LoadedPage {
                count: if grouped {
                    page.groups.len()
                } else {
                    page.items.len()
                },
                total: page.total,
                next: page.next,
                offset,
                totals,
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth,
                path,
                groups: grouped.then_some(page.groups),
                parents,
            })
        })
    }
}

impl<E: Model> GridSource for DataGrid<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self, view: Option<&DataGridView>) -> DataQuery {
        let page = match self.paging {
            Paging::Infinite { batch } => PageRequest::After {
                cursor: None,
                limit: batch,
            },
            other => PageRequest::Offset {
                offset: 0,
                limit: other.size(),
            },
        };
        DataQuery {
            page,
            with_total: !matches!(self.paging, Paging::Infinite { .. }),
            aggregates: self.aggregate_requests(),
            sort: view
                .map(|v| v.sort.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| self.default_sort.clone()),
            filters: view
                .map(|v| v.filters.clone())
                .unwrap_or_default(),
            ..DataQuery::default()
        }
    }

    fn clean_view(&self, view: DataGridView) -> DataGridView {
        self.clean(view)
    }

    fn head_html(&self, r: &Render, view: Option<&DataGridView>) -> String {
        self.head_row(r, &self.layout(view))
    }

    fn find_row(&self, ctx: Ctx, key: String) -> BoxFuture<Result<LoadedPage>> {
        let Some(i) = Self::key_index() else {
            return Box::pin(async { Err(crate::action::Error::NotFound) });
        };
        let meta = &E::fields()[i];
        let filter = match meta.kind {
            FieldKind::Id | FieldKind::Integer => Filter::range(meta.name, Some(&key), Some(&key)),
            _ => Filter {
                field: meta.name.to_string(),
                equals: Some(key),
                ..Filter::default()
            },
        };
        let mut query = DataQuery {
            page: PageRequest::Offset {
                offset: 0,
                limit: 5,
            },
            filters: vec![filter],
            time_zone: ctx.time_zone(),
            ..DataQuery::default()
        };
        self.scoped(&mut query);
        let fetch = self.fetch;
        Box::pin(async move {
            let page = fetch(ctx, query).await?;
            Ok(LoadedPage {
                count: page.items.len(),
                total: page.total,
                next: None,
                offset: 0,
                totals: Vec::new(),
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn bulk_call(&self, index: usize, keys: Vec<serde_json::Value>) -> Option<RowCall> {
        let b = self.bulk.get(index)?;
        Some(RowCall {
            meta: b.meta,
            run: (b.run)(keys)?,
        })
    }

    fn bulk_meta(&self, index: usize) -> Option<ActionMeta> {
        self.bulk.get(index).map(|b| b.meta)
    }

    fn key_json(&self, key: &str) -> Option<serde_json::Value> {
        let meta = &E::fields()[Self::key_index()?];
        match meta.kind {
            FieldKind::Id | FieldKind::Integer => key
                .parse::<i64>()
                .ok()
                .map(serde_json::Value::from),
            _ => Some(serde_json::Value::String(key.to_string())),
        }
    }

    fn page_keys(&self, page: &LoadedPage) -> Vec<serde_json::Value> {
        let Some(i) = Self::key_index() else {
            return Vec::new();
        };
        page.items
            .downcast_ref::<Vec<E>>()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r.value_at(i))
                    .filter_map(|v| serde_json::to_value(v).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn export_access(&self) -> Option<crate::action::Access> {
        self.export.map(|e| e.0)
    }

    fn export_limit(&self) -> usize {
        self.export.map_or(0, |e| e.1)
    }

    fn csv_head(&self, r: &Render, view: Option<&DataGridView>) -> String {
        let cols = self.layout(view);
        let line: Vec<String> = cols
            .iter()
            .map(|s| csv_cell(&self.columns[s.index].label_plain(r)))
            .collect();
        format!("{}\r\n", line.join(","))
    }

    fn csv_rows(&self, page: &LoadedPage, r: &Render) -> String {
        let cols = self.layout(page.view.as_ref());
        let Some(rows) = page.items.downcast_ref::<Vec<E>>() else {
            return String::new();
        };
        let mut out = String::new();
        for row in rows {
            let line: Vec<String> = cols
                .iter()
                .map(|s| {
                    let c = &self.columns[s.index];
                    let v = (c.value)(row);
                    csv_cell(&if c.meta.mask.is_some() {
                        masked(c.meta, &v)
                    } else {
                        plain(&v, &c.meta.kind, r)
                    })
                })
                .collect();
            out.push_str(&line.join(","));
            out.push_str("\r\n");
        }
        out
    }

    fn export_columns(&self, r: &Render, view: Option<&DataGridView>) -> Vec<ExportColumn> {
        self.layout(view)
            .iter()
            .map(|s| {
                let meta = &self.columns[s.index].meta;
                let numeric = matches!(
                    meta.kind,
                    FieldKind::Integer | FieldKind::Money | FieldKind::Decimal
                ) && meta.mask.is_none();
                ExportColumn {
                    label: r
                        .i18n
                        .text(r.locale, &meta.label_text()),
                    width: s.width,
                    numeric,
                }
            })
            .collect()
    }

    fn export_text(&self, page: &LoadedPage, r: &Render) -> Vec<Vec<String>> {
        let cols = self.layout(page.view.as_ref());
        let Some(rows) = page.items.downcast_ref::<Vec<E>>() else {
            return Vec::new();
        };
        rows.iter()
            .map(|row| {
                cols.iter()
                    .map(|s| {
                        let c = &self.columns[s.index];
                        let v = (c.value)(row);
                        if c.meta.mask.is_some() {
                            masked(c.meta, &v)
                        } else {
                            plain(&v, &c.meta.kind, r)
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn export_values(&self, page: &LoadedPage, r: &Render) -> Vec<Vec<Value>> {
        let cols = self.layout(page.view.as_ref());
        let Some(rows) = page.items.downcast_ref::<Vec<E>>() else {
            return Vec::new();
        };
        rows.iter()
            .map(|row| {
                cols.iter()
                    .map(|s| {
                        let c = &self.columns[s.index];
                        match (c.value)(row).into_owned() {
                            v if c.meta.mask.is_some() => Value::Text(masked(c.meta, &v)),
                            v @ Value::Choice(_) => Value::Text(plain(&v, &c.meta.kind, r)),
                            Value::DateTime(d) => Value::LocalDateTime(d.local(r.tz)),
                            v => v,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn edit_call(
        &self,
        page: &LoadedPage,
        key: &str,
        values: &serde_json::Map<String, serde_json::Value>,
        locale: &str,
        tz: TimeZone,
    ) -> Result<RowCall> {
        use crate::action::Error;
        let save = self
            .save
            .as_ref()
            .ok_or(Error::NotFound)?;
        let i = Self::key_index().ok_or(Error::NotFound)?;
        let rows = page
            .items
            .downcast_ref::<Vec<E>>()
            .ok_or(Error::NotFound)?;
        let row = rows
            .iter()
            .find(|r| {
                r.value_at(i)
                    .is_some_and(|v| key_text(&v) == key)
            })
            .ok_or(Error::NotFound)?;
        let editable = self.editable_fields();
        let fields = E::fields();
        let allowed = |name: &str| {
            fields
                .iter()
                .enumerate()
                .any(|(j, f)| f.name == name && (f.version || editable.contains(&j)))
        };
        if values.is_empty() || values.keys().any(|k| !allowed(k)) {
            return Err(Error::Forbidden);
        }
        let valid =
            crate::form::Form::with_fields(row.clone(), &editable).read(values, locale, tz)?;
        Ok(RowCall {
            meta: save.meta,
            run: (save.run)(valid),
        })
    }

    fn row_html(&self, page: &LoadedPage, key: &str, r: &Render) -> Option<String> {
        let i = Self::key_index()?;
        let rows = page.items.downcast_ref::<Vec<E>>()?;
        let row = rows.iter().find(|row| {
            row.value_at(i)
                .is_some_and(|v| key_text(&v) == key)
        })?;
        Some(self.rows(
            std::slice::from_ref(row),
            r,
            &self.layout(page.view.as_ref()),
            &Branch::default(),
        ))
    }

    fn row_call(&self, page: &LoadedPage, key: &str, item: &str) -> Option<RowCall> {
        let i = Self::key_index()?;
        let rows = page.items.downcast_ref::<Vec<E>>()?;
        let row = rows.iter().find(|r| {
            r.value_at(i)
                .is_some_and(|v| key_text(&v) == key)
        })?;
        if let Some(path) = item.strip_prefix('m') {
            return self.row_menu.as_ref()?(row).take(path);
        }
        let index: usize = item.strip_prefix('a')?.parse().ok()?;
        let action = self.row_actions.get(index)?;
        Some(RowCall {
            meta: action.meta,
            run: (action.run)(row),
        })
    }

    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        self.allow(&mut query, Some(&ctx));
        self.scoped(&mut query);
        query.time_zone = ctx.time_zone();
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
                totals: page.totals,
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn fetch_at(
        &self,
        ctx: Ctx,
        query: DataQuery,
        path: Vec<Option<String>>,
    ) -> BoxFuture<Result<LoadedPage>> {
        if self.nested() {
            self.fetch_nested(ctx, query, path)
        } else {
            self.fetch(ctx, query)
        }
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        self.rows_html_erased(page, r)
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn foot_html(&self, page: &LoadedPage, r: &Render) -> Option<String> {
        let has = self
            .columns
            .iter()
            .any(|c| !c.aggregates.is_empty());
        (has && !page.totals.is_empty())
            .then(|| self.foot_row(r, &page.totals, &self.layout(page.view.as_ref())))
    }

    fn fields(&self) -> &'static [FieldMeta] {
        E::fields()
    }

    fn sensitive_model(&self) -> Option<&'static str> {
        E::SENSITIVE.then_some(E::NAME)
    }

    /// Reveals a masked column of one row. The row is loaded like a row action's, through the
    /// grid's scope, so only rows the grid shows can be revealed.
    fn reveal(
        &self,
        ctx: Ctx,
        key: String,
        field: String,
    ) -> BoxFuture<Result<Option<crate::privacy::Revealed>>> {
        if !self
            .columns
            .iter()
            .any(|c| c.meta.name == field && c.meta.mask.is_some())
        {
            return Box::pin(async { Ok(None) });
        }
        let find = self.find_row(ctx, key.clone());
        Box::pin(async move {
            let page = find.await?;
            let rows = page.items.downcast_ref::<Vec<E>>();
            let row = rows.and_then(|rows| {
                rows.iter().find(|r| {
                    crate::audit::RecordRef::of(*r)
                        .id
                        .as_deref()
                        == Some(key.as_str())
                })
            });
            Ok(row.and_then(|row| crate::privacy::reveal_in(row, &field)))
        })
    }

    fn tool_schema(&self) -> serde_json::Value {
        let fields: Vec<&str> = self
            .columns
            .iter()
            .map(|c| c.meta.name)
            .collect();
        let filterable: Vec<&str> = self
            .columns
            .iter()
            .filter(|c| c.filter)
            .map(|c| c.meta.name)
            .collect();
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
                    "description": "Every set condition must hold. Numbers as 1234.56, dates as YYYY-MM-DD (a whole day in the user's time zone for moments), times as HH:MM.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "field": { "type": "string", "enum": filterable },
                            "contains": { "type": "string" },
                            "starts_with": { "type": "string" },
                            "equals": { "type": "string" },
                            "from": { "type": "string" },
                            "to": { "type": "string" },
                            "any_of": { "type": "array", "items": { "type": "string" } },
                            "is": { "type": "boolean" }
                        },
                        "required": ["field"]
                    }
                },
                "with_total": { "type": "boolean" },
                "aggregates": {
                    "type": "array",
                    "description": "Column totals over all matching rows. Only the totals the grid shows are computed.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "field": { "type": "string", "enum": fields },
                            "op": { "type": "string", "enum": ["sum", "avg", "min", "max", "count"] }
                        },
                        "required": ["field", "op"]
                    }
                }
            }
        })
    }
}

impl<E: Model> Component for DataGrid<E> {
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
                "aggregates": c.aggregates,
                "pin": c.pin,
                "hidden": c.hidden,
            })).collect::<Vec<_>>(),
            "row_actions": self.row_actions.iter().map(|a| json!({ "label": a.label, "action": a.meta })).collect::<Vec<_>>(),
            "detail": self.detail.as_ref().map(|d| d.screen),
            "group_by": self.group_by.iter().map(|m| m.name).collect::<Vec<_>>(),
            "tree": self.tree.map(|m| m.name),
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }
}

impl<E: Model> From<DataGrid<E>> for Node {
    fn from(g: DataGrid<E>) -> Self {
        Node::new(g)
    }
}

/// Whole number with the locale's group separator: `4.680` in `tr`, `4,680` in `en`.
fn grouped(n: i64, locale: &str) -> String {
    let mut out = String::new();
    crate::render::push_grouped(&mut out, n, locale);
    out
}

/// Most sort keys one query may carry.
pub const MAX_SORT: usize = 4;

/// Filter input the runtime shows for a column kind.
fn filter_kind(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Integer | FieldKind::Id => "number",
        FieldKind::Money | FieldKind::Decimal => "money",
        FieldKind::Date => "date",
        FieldKind::DateTime | FieldKind::LocalDateTime => "datetime",
        FieldKind::Time => "time",
        FieldKind::Choice(_) => "choice",
        FieldKind::Bool => "bool",
        FieldKind::Text | FieldKind::Custom { .. } | FieldKind::Choices(_) => "text",
    }
}

/// Localized texts the runtime needs for grid controls, sent once on the grid root.
fn grid_texts(r: &Render) -> serde_json::Value {
    let keys = [
        "filter",
        "from",
        "to",
        "all",
        "contains",
        "starts_with",
        "equals",
        "match",
        "selected",
        "clear",
        "sorted",
    ];
    let mut map = serde_json::Map::new();
    for k in keys {
        map.insert(
            k.to_string(),
            json!(
                r.i18n
                    .text(r.locale, &Text::key(format!("zeyn.grid.filter.{k}")))
            ),
        );
    }
    let menu = [
        "options",
        "resize",
        "sort_asc",
        "sort_desc",
        "sort_clear",
        "pin_start",
        "pin_end",
        "unpin",
        "move_prev",
        "move_next",
        "hide",
        "columns",
        "reset",
        "filter",
    ];
    for k in menu {
        map.insert(
            format!("menu_{k}"),
            json!(
                r.i18n
                    .text(r.locale, &Text::key(format!("zeyn.grid.menu.{k}")))
            ),
        );
    }
    map.insert(
        "done".into(),
        json!(
            r.i18n
                .text(r.locale, &Text::key("zeyn.grid.done"))
        ),
    );
    for k in [
        "saved",
        "undo",
        "undone",
        "empty",
        "error",
        "retry",
        "copied",
        "details",
        "loading",
        "load_more",
        "copied_cells",
        "paste_confirm",
        "pasted",
        "paste_failed",
        "paste_readonly",
        "paste_too_many",
    ] {
        map.insert(
            k.into(),
            json!(
                r.i18n
                    .text(r.locale, &Text::key(format!("zeyn.grid.{k}")))
            ),
        );
    }
    for k in ["title", "cancel"] {
        map.insert(
            format!("confirm_{k}"),
            json!(
                r.i18n
                    .text(r.locale, &Text::key(format!("zeyn.ui.confirm.{k}")))
            ),
        );
    }
    map.insert(
        "yes".into(),
        json!(
            r.i18n
                .text(r.locale, &Text::key("zeyn.yes"))
        ),
    );
    map.insert(
        "no".into(),
        json!(
            r.i18n
                .text(r.locale, &Text::key("zeyn.no"))
        ),
    );
    serde_json::Value::Object(map)
}
