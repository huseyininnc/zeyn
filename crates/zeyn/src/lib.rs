//! # Zeyn Framework
//!
//! A Rust web framework for data heavy applications. AI agents write the application code;
//! people read it, review it and change it. The language is kept small, typed and has one way
//! to do each thing.
//!
//! ```
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     #[rule(required, max_len = 20)]
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> {
//!     Ok(Page::default())
//! }
//!
//! #[screen(route = "/products", permission = "stock.product.read")]
//! pub fn product_list() -> Screen {
//!     Screen::new("Products").body(DataGrid::of(search_products).column(Product::code).filter())
//! }
//! ```
//!
//! ## Mistakes that do not compile
//!
//! An action without access:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action]
//! pub async fn delete_product(_id: i64) -> Result<()> { Ok(()) }
//! ```
//!
//! A text rule on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Filter {
//!     #[rule(max_len = 5)]
//!     pub active: bool,
//! }
//! ```
//!
//! A form whose submit action does not take `Valid<M>`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn form() -> Node {
//!     Form::of(Product::default()).field(Product::code).submit("Save", search_products).into()
//! }
//! ```
//!
//! A total on a text column:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).sum().into() }
//! ```
//!
//! An average on a text column, and a minimum on a yes/no column:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).avg().into() }
//! ```
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub active: bool,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::active).min().into() }
//! ```
//!
//! A badge on a column that is not a `#[choice]` enum:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//! use zeyn::Tone;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).badge(|_| Tone::Info).into() }
//! ```
//!
//! A row action whose input is not built from the row the way the action takes it:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn archive_product(_id: ProductId) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).row_action("Archive", archive_product, |p| p.code.clone()).into() }
//! ```
//!
//! A bulk action whose input does not hold keys of the given key field:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn archive_products(_codes: Vec<String>) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).bulk_action("Archive", archive_products, Product::id).into() }
//! ```
//!
//! An inline edit save action that does not take `Valid<M>`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn save_code(_p: Product) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).editable().edit_with(save_code).into() }
//! ```
//!
//! A menu item for a screen without a route:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(permission = "stock.product.write")]
//! pub fn product_form() -> Screen { Screen::window("Product") }
//!
//! fn feature() -> Feature {
//!     Feature::new("stock").screen(product_form).menu(|m| m.group("Stock", Icon::Box).item("Product", product_form))
//! }
//!
//! fn main() {
//!     let _ = feature();
//! }
//! ```
//!
//! This check runs when the code is generated, so `cargo check` alone may not report it.
//! [`Feature::check`] reports the same problem at startup.
//!
//! A key on a model that is not an entity:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Search {
//!     #[key]
//!     pub id: i64,
//! }
//! ```
//!
//! A `scale` rule on a field that is not a `Decimal`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Price {
//!     #[rule(scale = 2)]
//!     pub amount: i64,
//! }
//! ```
//!
//! A `mask` rule on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Contact {
//!     #[rule(mask = "(999) 999 99 99")]
//!     pub phone: i64,
//! }
//! ```
//!
//! A mask on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Card {
//!     #[mask(last4)]
//!     pub number: i64,
//! }
//! ```
//!
//! An unknown action option (the AI options are `ai = "..."`, `ai = false` and `confirm`):
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai_hidden)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `ai = true`, which is already the default:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = true)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! Empty guidance for models:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = " ")]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `confirm` on an action that models never see:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = false, confirm)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `confirm` on a screen instead of the action that writes:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(route = "/products", permission = "stock.product.read", confirm)]
//! pub fn product_list() -> Screen { Screen::new("Products") }
//! ```
//!
//! ## Extension points
//!
//! - [`Component`]: any type that renders HTML and describes itself in the schema can go on a screen.
//! - [`FieldType`]: new field types; use [`FieldKind::Custom`] to name a client side editor.
//! - Models, actions and screens from other crates are used like local ones.

extern crate self as zeyn;

#[cfg(feature = "accounts")]
pub mod accounts;
mod action;
mod api;
#[cfg(feature = "assistant")]
pub mod assistant;
pub mod audit;
#[cfg(feature = "server")]
pub mod auth;
mod command;
mod ctx;
mod decimal;
mod feature;
mod form;
mod grid;
mod i18n;
mod import;
#[cfg(feature = "server")]
mod jobs;
#[cfg(feature = "email")]
mod mail;
mod media;
mod menu;
mod model;
#[cfg(feature = "pdf")]
mod pdf;
mod platform;
mod privacy;
#[cfg(feature = "redis")]
mod redis_store;
pub mod render;
mod screen;
#[cfg(feature = "server")]
mod security;
#[cfg(feature = "server")]
pub mod server;
/// Data layer for SQL databases (`postgres` and `oracle` features).
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub mod sql;
mod time;
pub mod ui;
mod value;
mod workflow;
#[cfg(feature = "xlsx")]
mod xlsx;

#[allow(deprecated)]
pub use action::GridQuery;
pub use action::{
    Access, Action, ActionMeta, Aggregate, AggregateRequest, Ai, Cursor, DEFAULT_PAGE_SIZE, DataQuery, Error, Filter,
    MAX_FILTER_CHOICES, MAX_FILTER_TEXT, MAX_FILTERS, MAX_PAGE_SIZE, Page, PageRequest, Result, Sort, Total,
};
pub use api::{ApiEntry, ApiInput, ApiOutput, ApiResult, model_schema};
#[cfg(feature = "assistant")]
pub use assistant::Assistant;
pub use audit::{FieldChange, FieldRef, RecordRef, Trail};
#[cfg(feature = "server")]
pub use auth::RequireMfa;
#[cfg(feature = "oidc")]
pub use auth::{Algorithm, ClaimMap, JwtAuth, OidcAuth};
#[cfg(feature = "totp")]
pub use auth::{MemoryTotpStore, Totp, TotpRecord, TotpStore};
#[allow(deprecated)]
pub use command::RESERVED_SHORTCUTS;
pub use command::{
    BROWSER_DEFAULTS, BROWSER_RESERVED, BrowserShortcut, Command, PALETTE_SHORTCUT, browser_shortcut,
    normalize_shortcut,
};
pub use command::{MAX_SEARCH_HITS, MAX_SEARCH_QUERY, Search, SearchHit};
pub use ctx::{BoxFuture, Ctx, Dep, Identity, Inject, IntoScreen, Outcome, Service, Services};
pub use decimal::{DEFAULT_PRECISION, DEFAULT_SCALE, Decimal, MAX_PRECISION, MAX_SCALE};
pub use feature::{Feature, Guard, GuardResult, Menu, MenuGroup, MenuItem, Route, ScreenEntry, SignedIn};
pub use form::{Form, FormAction, FormRow, SubmitForm};
pub use grid::{
    BulkAction, COLUMN_WIDTH, CellRenderer, ColumnView, DEFAULT_EXPORT_LIMIT, DataGrid, DataGridColumn, DataGridView,
    DataGridViewStore, DataMode, ExportColumn, MAX_BULK, MAX_PINNED, MAX_SORT, MemoryViewStore, Paging, Pin, RowStyle,
    Tone, position_cursor, total,
};
#[allow(deprecated)]
pub use grid::{Grid, GridColumn, GridMode};
pub use i18n::{I18n, NumberFormat, Text};
pub use import::{
    Checked, CsvError, DEFAULT_MAX_ROWS, HARD_MAX_ROWS, Import, ImportFile, ImportSource, MAX_CSV_BYTES, RowError,
    check_csv, check_file, error_report, parse_csv,
};
#[cfg(feature = "server")]
pub use jobs::{JobState, JobStatus, Jobs, Schedule};
#[cfg(feature = "email")]
pub use mail::{MAIL_JOB, MAX_ATTEMPTS, MAX_QUEUED, Mail, Mailer};
pub use media::{
    ByteRange, MAX_MEDIA_ID, MAX_POSITION_SECONDS, MEDIA_CHUNK, Media, MediaEntry, MediaReader, MediaSource,
    media_type, valid_media_id,
};
pub use menu::ContextMenu;
pub use model::{__rules, Entity, Field, FieldError, FieldMeta, Model, Rule, Valid, ValidationErrors};
#[cfg(feature = "pdf")]
pub use pdf::{MAX_PDF_DATA, MAX_PDF_ROWS, Pdf, PdfLink, PdfSource, PdfTemplate};
pub use platform::{
    Config, ConfigSource, EnvSource, FixedLicense, JsonFileSource, License, LicenseSource, Licensed, PerTenant, Secret,
    StaticSource, Tenant, TenantRequired,
};
#[cfg(feature = "signed-licenses")]
pub use platform::{LicenseError, SignedLicenseFile};
pub use privacy::{MaskStyle, Masking, Revealed, can_unmask, masked_text, reveal_in};
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
pub use render::Render;
pub use screen::{
    Component, Effect, Export, FormSource, GridSource, Icon, LoadedPage, Node, Params, RowCall, Screen, ScreenDef,
    ScreenMeta, ToolButton, Toolbar, UiCtx, Window,
};
#[cfg(feature = "server")]
pub use security::{IpRange, MemoryRateStore, RateKey, RateLimit, RateStore, client_ip, parse_ranges};
#[cfg(feature = "server")]
#[allow(deprecated)]
pub use server::Agents;
/// Realtime channel: WebSocket rooms with Y-CRDT documents and change topics.
#[cfg(feature = "realtime")]
pub use server::realtime;
#[cfg(feature = "server")]
pub use server::{
    ApiDocs, App, AuditEntry, AuditLog, AuditSink, Authenticator, BasicCredentials, Call, CallKind, CallOutcome,
    DevAuth, ForwardAuth, HeaderTenant, Interceptor, Mcp, MemoryAudit, Plugin, StaticCredentials, StderrAudit,
    SubdomainTenant, TenantResolver, Theme, TimePolicy, probe,
};
pub use time::{Date, DateTime, LocalDateTime, LocalTimeError, Time, TimeZone};
pub use time::{DateWords, range_presets};
pub use value::{
    ChoiceOption, ChoiceType, Currency, FieldKind, FieldType, IsBool, IsMoment, Money, SupportsMinMax,
    SupportsMinMaxTotal, SupportsRequired, SupportsScale, SupportsSum, SupportsText, Value,
};
pub use workflow::{
    MAX_COMMENT, Transition, TransitionRecord, TransitionRequest, Transitions, Workflow, WorkflowState,
};
#[cfg(feature = "xlsx")]
pub use xlsx::{MAX_XLSX_BYTES, XlsxReadError, XlsxSheet};
pub use zeyn_macros::{action, choice, entity, model, params, screen, t};

#[doc(hidden)]
pub use serde as __serde;
#[doc(hidden)]
pub use serde_json as __serde_json;

/// Everything application code needs: `use zeyn::prelude::*;`
pub mod prelude {
    #[cfg(feature = "server")]
    #[allow(deprecated)]
    pub use crate::Agents;
    pub use crate::ui::{
        ActionButton, Alert, Audio, Badge, Breadcrumb, Card, Chart, Control, DateOptions, DescriptionList, Dock,
        EmptyState, LookupItem, LookupQuery, MenuBar, Panel, Progress, PropertyInspector, Row, Skeleton, Split, Stack,
        StatusBar, Tabs, Timeline, Toolbox, Tree, TreeItem, TreeQuery, Video,
    };
    pub use crate::{
        Action, Command, ContextMenu, Ctx, Currency, DataGrid, DataMode, DataQuery, Date, DateTime, Decimal, Effect,
        Entity, Error, Export, Feature, Field, Form, GuardResult, Icon, Identity, Import, Licensed, LocalDateTime,
        Model, Money, Node, Page, Paging, Result, RowStyle, Screen, ScreenDef, Secret, Service, SignedIn, Tenant,
        TenantRequired, Text, Time, TimeZone, Tone, TransitionRequest, Transitions, UiCtx, Valid, ValidationErrors,
        Window, Workflow, action, choice, entity, model, params, screen, t,
    };
    #[cfg(feature = "server")]
    pub use crate::{
        ApiDocs, App, AuditLog, DevAuth, Mcp, MemoryAudit, Plugin, RateLimit, StaticCredentials, StderrAudit, Theme,
        TimePolicy,
    };
    #[allow(deprecated)]
    pub use crate::{Grid, GridMode, GridQuery};
}
