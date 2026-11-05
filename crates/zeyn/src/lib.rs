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
//! The compiler refuses these, with an error message that says what to change. Each one has a
//! `compile_fail` test in `src/compile_fail.rs`:
//!
//! - an action without access
//! - a shared cache on a screen that is not public, and one longer than a day
//! - a text rule on a field that is not text
//! - a form whose submit action does not take `Valid<M>`
//! - a route segment that names no field of the screen's params
//! - a route with segments on a screen that takes no params
//! - a total on a text column
//! - an average on a text column, and a minimum on a yes/no column
//! - a badge on a column that is not a `#[choice]` enum
//! - a row action whose input is not built from the row the way the action takes it
//! - a bulk action whose input does not hold keys of the given key field
//! - an inline edit save action that does not take `Valid<M>`
//! - a menu item for a screen without a route
//! - a key on a model that is not an entity
//! - a `scale` rule on a field that is not a `Decimal`
//! - a `Vec` of something that is neither a line model nor a `#[choice]` enum
//! - a `mask` rule on a field that is not text
//! - a mask on a field that is not text
//! - an unknown action option (the AI options are `ai = "..."`, `ai = false` and `confirm`)
//! - empty guidance for models
//!
//! ## Extension points
//!
//! - [`Component`]: any type that renders HTML and describes itself in the schema can go on a screen.
//! - [`FieldType`]: new field types; use [`FieldKind::Custom`] to name a client side editor.
//! - Models, actions and screens from other crates are used like local ones.

#[cfg(doctest)]
mod compile_fail;

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
mod csp;
mod ctx;
mod decimal;
mod feature;
mod files;
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
/// Head tags for search engines and the sitemap.
pub mod seo;
#[cfg(feature = "server")]
pub mod server;
mod site;
mod slug;
/// Data layer for SQL databases (`postgres` and `oracle` features).
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub mod sql;
mod time;
pub mod ui;
mod value;
mod workflow;
#[cfg(feature = "xlsx")]
mod xlsx;

pub use action::{
    Access, Action, ActionMeta, Aggregate, AggregateRequest, Ai, Cursor, DEFAULT_PAGE_SIZE,
    DataQuery, Error, Filter, MAX_FILTER_CHOICES, MAX_FILTER_TEXT, MAX_FILTERS, MAX_PAGE_SIZE,
    Page, PageRequest, Result, Sort, Total,
};
pub use api::{ApiEntry, ApiInput, ApiOutput, ApiResult, model_schema};
#[cfg(feature = "assistant")]
pub use assistant::Assistant;
pub use audit::{ExportRecord, FieldChange, FieldRef, RecordRef, Trail};
#[cfg(feature = "server")]
pub use auth::RequireMfa;
#[cfg(feature = "oidc")]
pub use auth::{Algorithm, ClaimMap, JwtAuth, OidcAuth};
#[cfg(feature = "totp")]
pub use auth::{MemoryTotpStore, Totp, TotpRecord, TotpStore};
pub use command::{
    BROWSER_DEFAULTS, BROWSER_RESERVED, BrowserShortcut, Command, PALETTE_SHORTCUT,
    browser_shortcut, normalize_shortcut,
};
pub use command::{MAX_SEARCH_HITS, MAX_SEARCH_QUERY, Search, SearchHit};
pub use csp::{Csp, MAX_EMBED_URL, TRACKING_PARAMS, embed_url};
pub use ctx::{BoxFuture, Ctx, Dep, Identity, Inject, IntoScreen, Outcome, Service, Services};
pub use decimal::{DEFAULT_PRECISION, DEFAULT_SCALE, Decimal, MAX_PRECISION, MAX_SCALE};
pub use feature::{
    Feature, Guard, GuardResult, Menu, MenuGroup, MenuItem, Route, ScreenEntry, SignedIn,
};
#[cfg(feature = "server")]
pub use files::{
    DiskStore, FileChunks, FileEntry, FileMeta, FileScanner, FileStore, MemoryStore, ScanVerdict,
    StoredFile,
};
pub use files::{
    File, FileRules, FileType, MAX_FIELD_FILES, MAX_FILE_NAME, MAX_UPLOAD_BYTES, SNIFF_BYTES,
    UPLOAD_CHUNK, UploadValue, clean_file_name, valid_file_id,
};
pub use form::{Form, FormAction, FormRow, SubmitForm};
pub use grid::{
    BulkAction, COLUMN_WIDTH, CellRenderer, ColumnView, DEFAULT_EXPORT_LIMIT, DataGrid,
    DataGridColumn, DataGridView, DataGridViewStore, DataMode, ExportColumn, MAX_BULK, MAX_PINNED,
    MAX_SORT, MemoryViewStore, Paging, Pin, RowStyle, Tone, VIRTUAL_LIVE_ROWS,
    VIRTUAL_WIDE_COLUMNS, position_cursor, total,
};
pub use i18n::{I18n, NumberFormat, Text};
pub use import::{
    Checked, CsvError, DEFAULT_MAX_ROWS, HARD_MAX_ROWS, Import, ImportFile, ImportSource,
    MAX_CSV_BYTES, RowError, check_csv, check_file, error_report, parse_csv,
};
#[cfg(feature = "server")]
pub use jobs::{
    ClaimedJob, JOB_LEASE, JOB_POLL, JobState, JobStatus, JobStore, Jobs, MAX_BACKOFF_MS,
    MAX_JOB_PAYLOAD, MEMORY_JOB_CAPACITY, MemoryJobStore, NewJob, NextRun, Schedule,
};
#[cfg(all(feature = "server", any(feature = "postgres", feature = "oracle")))]
pub use jobs::{SqlJobStore, create_job_tables};
#[cfg(feature = "email")]
pub use mail::{MAIL_JOB, MAX_ATTEMPTS, MAX_QUEUED, Mail, Mailer};
pub use media::{
    ByteRange, MAX_MEDIA_ID, MAX_POSITION_SECONDS, MAX_PUBLIC_MEDIA_CACHE, MEDIA_CHUNK, Media,
    MediaEntry, MediaReader, MediaSource, media_type, valid_media_id,
};
pub use menu::ContextMenu;
pub use model::{
    __rules, Entity, Field, FieldError, FieldMeta, Model, Rule, Valid, ValidationErrors,
};
#[cfg(feature = "pdf")]
pub use pdf::{MAX_PDF_DATA, MAX_PDF_ROWS, Pdf, PdfLink, PdfSource, PdfTemplate};
pub use platform::{
    Config, ConfigSource, EnvSource, FixedLicense, JsonFileSource, License, LicenseSource,
    Licensed, PerTenant, Secret, StaticSource, Tenant, TenantRequired,
};
#[cfg(feature = "signed-licenses")]
pub use platform::{LicenseError, SignedLicenseFile};
#[doc(hidden)]
pub use privacy::output as __privacy;
pub use privacy::{MaskStyle, Masking, Revealed, can_unmask, masked_text, reveal_in};
#[cfg(feature = "redis")]
pub use redis_store::RedisStore;
pub use render::Render;
pub use screen::{
    __path_value, Component, Effect, Export, FormSource, GridSource, Icon, LoadedPage,
    MAX_SCREEN_CACHE, MAX_SEGMENT, MAX_TOASTS, Node, Params, PathValue, RowCall, Screen, ScreenDef,
    ScreenMeta, Toast, ToolButton, Toolbar, UiCtx, Window, route_has_segments, screen_url,
};
#[cfg(feature = "server")]
pub use security::{
    IpRange, MemoryRateStore, RateKey, RateLimit, RateStore, client_ip, parse_ranges,
};
/// Realtime channel: WebSocket rooms with Y-CRDT documents and change topics.
#[cfg(feature = "realtime")]
pub use server::realtime;
#[cfg(feature = "server")]
pub use server::{
    ApiDocs, App, AuditEntry, AuditLog, AuditSink, Authenticator, BasicCredentials, Call, CallKind,
    CallOutcome, DevAuth, ForwardAuth, HeaderTenant, Interceptor, Mcp, MemoryAudit, Plugin,
    READY_CACHE, StaticCredentials, StderrAudit, SubdomainTenant, TenantResolver, Theme,
    TimePolicy, probe,
};
pub use site::{BottomNavItem, FooterColumn, Shell, SiteFooter, SiteHeader, SiteShell};
pub use slug::Slug;
pub use time::{Date, DateTime, LocalDateTime, LocalTimeError, Time, TimeZone};
pub use time::{DateWords, range_presets};
#[doc(hidden)]
pub use value::__choices_value;
pub use value::{
    ChoiceOption, ChoiceType, Currency, FieldKind, FieldType, IsBool, IsMoment, Money,
    SupportsMinMax, SupportsMinMaxTotal, SupportsRequired, SupportsScale, SupportsSum,
    SupportsText, Value, VecItem,
};
pub use workflow::{
    MAX_COMMENT, Transition, TransitionRecord, TransitionRequest, Transitions, Workflow,
    WorkflowState,
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
    pub use crate::seo::{Robots, SitemapPart, SitemapUrl};
    #[cfg(feature = "markdown")]
    pub use crate::ui::Markdown;
    pub use crate::ui::{
        ActionButton, Alert, AlphaIndex, Audio, AutoGrid, Avatar, Badge, Breadcrumb, Busy, Card,
        CardGrid, Chart, Chips, Control, DateOptions, DescriptionList, Dock, Embed,
        EmbedPermission, EmptyState, ExternalLink, Heading, Hero, Image, Kpi, LinkButton, List,
        LookupItem, LookupQuery, MenuBar, MenuButton, MetaLine, Panel, Paragraph, Popover,
        PrevNext, Progress, PropertyInspector, Rail, Reactions, RelativeTime, Row, SearchBox,
        SearchParams, Share, Skeleton, Split, Spoiler, Stack, StatusBar, Tabs, Timeline, Toolbox,
        Tree, TreeItem, TreeQuery, Video, WeekSchedule,
    };
    pub use crate::{
        Action, Command, ContextMenu, Ctx, Currency, DataGrid, DataMode, DataQuery, Date, DateTime,
        Decimal, Effect, Entity, Error, Export, Feature, Field, File, FileRules, FileType, Form,
        GuardResult, Icon, Identity, Import, Licensed, LocalDateTime, Model, Money, Node, Page,
        Paging, Result, RowStyle, Screen, ScreenDef, Secret, Service, Shell, SignedIn, Slug,
        Tenant, TenantRequired, Text, Time, TimeZone, Toast, Tone, TransitionRequest, Transitions,
        UiCtx, Valid, ValidationErrors, Window, Workflow, action, choice, entity, model, params,
        screen, t,
    };
    #[cfg(feature = "server")]
    pub use crate::{
        ApiDocs, App, AuditLog, DevAuth, DiskStore, Mcp, MemoryAudit, MemoryStore, Plugin,
        RateLimit, StaticCredentials, StderrAudit, Theme, TimePolicy,
    };
}
