//! HTTP server: page shell, screen, grid, form, language and agent endpoints.
//!
//! The protocol is described in `docs/runtime-protocol.md`.

use crate::action::{ActionMeta, DataQuery, Error};
use crate::ctx::{Ctx, Identity, Service, Services};
use crate::feature::{Feature, ScreenEntry};
use crate::grid::DataGridView;
use crate::i18n::{I18n, Text};
use crate::platform::{Config, License, LicenseSource, Tenant};
use crate::render::{Render, escape};
use crate::screen::{GridSource, LoadedPage, Screen, ScreenDef};
use crate::security::{IpRange, MemoryRateStore, RateKey, RateLimit, RateStore, client_ip, parse_ranges};
use crate::time::{DateTime, TimeZone};
use crate::ui::NodePath;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json, json};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(feature = "accounts")]
pub(crate) mod accounts;
mod agent;
mod auth;
#[cfg(feature = "assistant")]
use agent::{Tool, ToolTarget};
mod data;
mod grid;
mod guard;
mod media;
mod ops;
mod session;
mod telemetry;
mod theme;

pub use theme::Theme;
#[cfg(feature = "email")]
mod mail;
#[cfg(feature = "pdf")]
mod pdf;
#[cfg(feature = "xlsx")]
mod xlsx;

pub use ops::{build_id, probe};

/// The browser runtime, served as one script at `/_zeyn/zeyn.js` so a page makes one request. The
/// source is one file per concern under `assets/zeyn/`, joined here in this order (later parts
/// use functions and constants of earlier ones):
///
/// - `core.js`: constants, requests, toasts, effects, windows
/// - `grid.js`: grid setup, loading, paging and virtual scrolling
/// - `grid-rows.js`: selection, bulk actions, inline edits, row items
/// - `grid-columns.js`: sort, saved views, header, pinning, resizing, column and context menus
/// - `grid-filters.js`: filter controls
/// - `grid-cells.js`: row details, cell ranges, copy and paste, column windowing, branches,
///   row keyboard
/// - `page.js`: shortcuts, forms, export, language, time zone and theme, WebMCP tools
/// - `events.js`: event delegation on `document` and start up
const JS: &str = concat!(
    include_str!("../assets/zeyn/core.js"),
    include_str!("../assets/zeyn/grid.js"),
    include_str!("../assets/zeyn/grid-rows.js"),
    include_str!("../assets/zeyn/grid-columns.js"),
    include_str!("../assets/zeyn/grid-filters.js"),
    include_str!("../assets/zeyn/grid-cells.js"),
    include_str!("../assets/zeyn/page.js"),
    include_str!("../assets/zeyn/events.js"),
);
const CSS: &str = include_str!("../assets/zeyn.css");
const NAV: &str = include_str!("../assets/nav.js");

#[cfg(feature = "assistant")]
mod assistant;
#[cfg(feature = "islands")]
mod islands;
mod pro;
#[cfg(feature = "realtime")]
pub mod realtime;
mod ui;

/// Finds out who is calling, usually from a session cookie or an `Authorization` header.
pub trait Authenticator: Send + Sync + 'static {
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity>;

    /// Async work before `authenticate`, once per request, such as fetching an issuer's signing
    /// keys. `None` (the default) when there is nothing to wait for, which costs nothing.
    fn prepare<'a>(
        &'a self,
        _headers: &'a HeaderMap,
    ) -> Option<std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'a>>> {
        None
    }

    /// True for development only authenticators; the server prints a warning at startup.
    fn is_development(&self) -> bool {
        false
    }
}

impl<F> Authenticator for F
where
    F: Fn(&HeaderMap) -> Option<Identity> + Send + Sync + 'static,
{
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        self(headers)
    }
}

/// Identity from a single sign on proxy in front of the app (Authentik, Keycloak with
/// oauth2-proxy, Azure Application Proxy and similar): the proxy signs the user in against the
/// central login and passes who they are in headers.
///
/// **Only use this when every request goes through that proxy.** Anyone who can reach the app
/// directly can send these headers themselves. Bind the app to a private address or check the
/// proxy's shared secret header with [`ForwardAuth::require_header`].
///
/// ```ignore
/// ForwardAuth::new("x-forwarded-user")
///     .groups("x-forwarded-groups")
///     .require_header("x-proxy-secret", secret.expose())
///     .role("stock-admins", ["stock.product.read", "stock.product.write"])
/// ```
pub struct ForwardAuth {
    user: &'static str,
    groups: Option<&'static str>,
    tenant: Option<&'static str>,
    time_zone: Option<&'static str>,
    auth_time: Option<&'static str>,
    shared: Option<(&'static str, String)>,
    roles: HashMap<String, Vec<String>>,
}

impl ForwardAuth {
    pub fn new(user_header: &'static str) -> Self {
        ForwardAuth {
            user: user_header,
            groups: None,
            tenant: None,
            time_zone: None,
            auth_time: None,
            shared: None,
            roles: HashMap::new(),
        }
    }

    /// Header with the user's groups, comma separated.
    pub fn groups(mut self, header: &'static str) -> Self {
        self.groups = Some(header);
        self
    }

    /// Header with the user's tenant.
    pub fn tenant(mut self, header: &'static str) -> Self {
        self.tenant = Some(header);
        self
    }

    /// Header with the user's time zone.
    pub fn time_zone(mut self, header: &'static str) -> Self {
        self.time_zone = Some(header);
        self
    }

    /// Header with the time the user signed in, as Unix seconds or RFC 3339. Needed by actions
    /// marked `reauth`.
    pub fn auth_time(mut self, header: &'static str) -> Self {
        self.auth_time = Some(header);
        self
    }

    /// Requests must carry this header with this exact value, set by the proxy.
    pub fn require_header(mut self, header: &'static str, value: &str) -> Self {
        self.shared = Some((header, value.to_string()));
        self
    }

    /// Permissions granted to members of a group.
    pub fn role<I, S>(mut self, group: &str, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.roles.entry(group.to_string()).or_default().extend(permissions.into_iter().map(Into::into));
        self
    }
}

impl Authenticator for ForwardAuth {
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        let text =
            |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|v| !v.is_empty());
        if let Some((name, value)) = &self.shared {
            if !text(name).is_some_and(|v| same(v, value)) {
                return None;
            }
        }
        let mut identity = Identity::user(text(self.user)?);
        if let Some(groups) = self.groups.and_then(text) {
            for group in groups.split(',').map(str::trim) {
                if let Some(perms) = self.roles.get(group) {
                    identity.permissions.extend(perms.iter().cloned());
                }
            }
        }
        identity.tenant = self.tenant.and_then(text).map(str::to_string);
        identity.time_zone = self.time_zone.and_then(text).map(str::to_string);
        identity.authenticated_at = self.auth_time.and_then(text).and_then(|t| match t.parse::<i64>() {
            Ok(secs) => secs.checked_mul(1000).map(DateTime::from_unix_ms),
            Err(_) => DateTime::parse(t),
        });
        Some(identity)
    }
}

/// Finds the tenant of a request. The returned id is checked with `Tenant::new`.
pub trait TenantResolver: Send + Sync + 'static {
    fn resolve(&self, headers: &HeaderMap) -> Option<String>;
}

impl<F> TenantResolver for F
where
    F: Fn(&HeaderMap) -> Option<String> + Send + Sync + 'static,
{
    fn resolve(&self, headers: &HeaderMap) -> Option<String> {
        self(headers)
    }
}

/// Tenant from a request header, such as `x-tenant`. Use only behind a gateway that sets it.
pub struct HeaderTenant(pub &'static str);

impl TenantResolver for HeaderTenant {
    fn resolve(&self, headers: &HeaderMap) -> Option<String> {
        headers.get(self.0).and_then(|v| v.to_str().ok()).map(|v| v.trim().to_lowercase())
    }
}

/// Tenant from the first label of the host: `acme.app.example.com` is `acme` for base
/// `app.example.com`.
pub struct SubdomainTenant(pub &'static str);

impl TenantResolver for SubdomainTenant {
    fn resolve(&self, headers: &HeaderMap) -> Option<String> {
        let host = headers.get(header::HOST)?.to_str().ok()?.split(':').next()?.to_lowercase();
        let label = host.strip_suffix(self.0)?.strip_suffix('.')?;
        (!label.contains('.')).then(|| label.to_string())
    }
}

/// Treats every request as the same identity. Never use it in production.
pub struct DevAuth(Identity);

impl DevAuth {
    /// Every request is a superuser with this name.
    pub fn superuser(name: &str) -> Self {
        DevAuth(Identity::superuser(name))
    }

    pub fn identity(identity: Identity) -> Self {
        DevAuth(identity)
    }
}

impl Authenticator for DevAuth {
    fn authenticate(&self, _headers: &HeaderMap) -> Option<Identity> {
        let mut identity = self.0.clone();
        identity.authenticated_at.get_or_insert_with(DateTime::now);
        Some(identity)
    }

    fn is_development(&self) -> bool {
        true
    }
}

/// Kind of call an interceptor sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    Page,
    Screen,
    DataGrid,
    Form,
    Api,
    /// A masked value shown in full on request.
    Reveal,
    /// Rows imported from a CSV file.
    Import,
    /// A media file or a saved play position; `screen` holds the media source name.
    Media,
    /// A PDF document rendered from an action's data.
    Document,
    /// A live channel opened (a WebSocket handshake); `screen` holds the document kind, or is
    /// empty for change topics.
    Live,
}

/// What is being called.
#[derive(Clone, Debug, Serialize)]
pub struct Call {
    pub kind: CallKind,
    pub screen: &'static str,
    pub action: Option<ActionMeta>,
    pub via_agent: bool,
}

/// How a call ended.
#[derive(Clone, Debug)]
pub struct CallOutcome {
    pub ok: bool,
    /// Error code such as `forbidden` or `validation`.
    pub error: Option<&'static str>,
    pub duration: Duration,
}

/// Runs around every screen, grid and form call, after access checks.
pub trait Interceptor: Send + Sync + 'static {
    /// Name in traces; the type name unless overridden.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Returning an error stops the call.
    fn before(&self, _ctx: &Ctx, _call: &Call) -> crate::Result<()> {
        Ok(())
    }

    fn after(&self, _ctx: &Ctx, _call: &Call, _outcome: &CallOutcome) {}
}

/// One audit record.
#[derive(Clone, Debug, Serialize)]
pub struct AuditEntry {
    pub at_unix_ms: u128,
    pub user: Option<String>,
    pub kind: CallKind,
    pub screen: &'static str,
    pub action: Option<&'static str>,
    pub ok: bool,
    pub error: Option<&'static str>,
    pub via_agent: bool,
    pub millis: u128,
    /// Model of the saved record, such as `Product`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<&'static str>,
    /// Key of the saved record as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// Changed fields with old and new values. Old values come from the record the server loaded.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<crate::audit::FieldChange>,
    /// Masked values the user was shown in full.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub revealed: Vec<crate::audit::FieldRef>,
    /// Sensitive records the user was shown.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<crate::audit::RecordRef>,
    /// Reason the user gave for opening a screen that asks for one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Workflow move: name, from, to and the user's comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<crate::workflow::TransitionRecord>,
    /// Export of sensitive records: model, format, row count and query.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export: Option<crate::audit::ExportRecord>,
}

/// Where audit records go.
pub trait AuditSink: Send + Sync + 'static {
    fn record(&self, entry: AuditEntry);
}

/// Keeps audit records in memory. For tests and demos.
#[derive(Clone, Default)]
pub struct MemoryAudit(Arc<Mutex<Vec<AuditEntry>>>);

impl MemoryAudit {
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.0.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

impl AuditSink for MemoryAudit {
    fn record(&self, entry: AuditEntry) {
        if let Ok(mut e) = self.0.lock() {
            e.push(entry);
        }
    }
}

/// Writes audit records to standard error as JSON lines.
pub struct StderrAudit;

impl AuditSink for StderrAudit {
    fn record(&self, entry: AuditEntry) {
        if let Ok(line) = serde_json::to_string(&entry) {
            eprintln!("{line}");
        }
    }
}

/// Interceptor that records every form submit, every call to an action marked `audit` and every
/// call made by an AI agent.
pub struct AuditLog<S: AuditSink>(pub S);

impl<S: AuditSink> Interceptor for AuditLog<S> {
    fn after(&self, ctx: &Ctx, call: &Call, outcome: &CallOutcome) {
        let trail = ctx.trail();
        let audited = call.via_agent
            || call.kind == CallKind::Form
            || call.kind == CallKind::Reveal
            || call.kind == CallKind::Import
            || call.action.is_some_and(|a| a.audit)
            || !trail.revealed.is_empty()
            || !trail.reads.is_empty()
            || trail.transition.is_some()
            || trail.export.is_some();
        if !audited {
            return;
        }
        self.0.record(AuditEntry {
            model: trail.record.as_ref().map(|r| r.model),
            record: trail.record.and_then(|r| r.id),
            changes: trail.changes,
            revealed: trail.revealed,
            reads: trail.reads,
            reason: trail.reason,
            transition: trail.transition,
            export: trail.export,
            at_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0),
            user: ctx.identity().user.clone(),
            kind: call.kind,
            screen: call.screen,
            action: call.action.map(|a| a.name),
            ok: outcome.ok,
            error: outcome.error,
            via_agent: call.via_agent,
            millis: outcome.duration.as_millis(),
        });
    }
}

/// Lets outside AI agents use the app: the MCP endpoint `/_zeyn/mcp` (JSON-RPC, needs an
/// `Authorization` header) and WebMCP tools in the browser (`GET /_zeyn/tools`).
///
/// Off unless set with [`App::mcp`]. The tools are built from the app itself: one search tool per
/// grid and one save tool per form of every routed screen, plus every published API action, with
/// descriptions from the `///` doc comments. Agents act as the signed in user: same permissions,
/// guards, interceptors and rate limits, every call audited with `via_agent`, masked fields always
/// masked. Actions and screens marked `ai = false` are never offered, and actions marked `confirm`
/// cannot be called over `/_zeyn/mcp` because nobody there can confirm them.
///
/// ```ignore
/// App::new("Shop").mcp(Mcp::read_only())          // agents may read
/// App::new("Shop").mcp(Mcp::on().webmcp(false))   // read and write, server endpoint only
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mcp {
    writes: bool,
    server: bool,
    browser: bool,
}

/// Old name of [`Mcp`]; kept for one release.
#[deprecated(since = "0.0.3", note = "renamed to `Mcp`; use `App::mcp(Mcp::on())` or `Mcp::read_only()`")]
pub type Agents = Mcp;

impl Mcp {
    /// No MCP endpoint and no WebMCP tools. The default.
    pub fn off() -> Self {
        Mcp::default()
    }

    /// Agents may search grids and call API actions that only read, not save forms.
    pub fn read_only() -> Self {
        Mcp { writes: false, server: true, browser: true }
    }

    /// Agents may also save forms and call API actions that write. Audit records mark these calls.
    pub fn on() -> Self {
        Mcp { writes: true, server: true, browser: true }
    }

    /// Same as [`Mcp::on`].
    pub fn read_write() -> Self {
        Mcp::on()
    }

    /// `false` turns off the `/_zeyn/mcp` endpoint and keeps the browser tools.
    pub fn server(mut self, on: bool) -> Self {
        self.server = on;
        self
    }

    /// `false` turns off the WebMCP tools (`GET /_zeyn/tools`) and keeps the endpoint.
    pub fn webmcp(mut self, on: bool) -> Self {
        self.browser = on;
        self
    }

    /// Whether agents may write.
    pub fn writes(&self) -> bool {
        self.writes
    }

    /// Whether anything is on.
    pub fn is_on(&self) -> bool {
        self.server || self.browser
    }
}

/// Checks a user name and password, for HTTP basic authentication of the API documentation.
/// Implement it to read the password from a database, a vault or anywhere else.
pub trait BasicCredentials: Send + Sync + 'static {
    fn verify(&self, user: &str, password: &str) -> crate::BoxFuture<bool>;
}

/// One fixed user name and password, compared in constant time.
pub struct StaticCredentials {
    user: String,
    password: crate::Secret<String>,
}

impl StaticCredentials {
    pub fn new(user: &str, password: crate::Secret<String>) -> Self {
        StaticCredentials { user: user.to_string(), password }
    }
}

impl BasicCredentials for StaticCredentials {
    fn verify(&self, user: &str, password: &str) -> crate::BoxFuture<bool> {
        let ok = same(user, &self.user) & same(password, self.password.expose());
        Box::pin(async move { ok })
    }
}

/// Who may read the API documentation (`openapi.json` and the reference page).
pub enum ApiDocs {
    /// Anyone who can reach the server.
    Open,
    /// HTTP basic authentication. Use only over HTTPS and together with a rate limit.
    Basic(Arc<dyn BasicCredentials>),
    /// Signed in users with this permission.
    Permission(&'static str),
}

impl ApiDocs {
    pub fn open() -> Self {
        ApiDocs::Open
    }

    pub fn basic(credentials: impl BasicCredentials) -> Self {
        ApiDocs::Basic(Arc::new(credentials))
    }

    pub fn permission(permission: &'static str) -> Self {
        ApiDocs::Permission(permission)
    }
}

/// Decodes standard base64; `None` on any invalid input.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let bytes = s.trim().trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        let take = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return None,
        };
        out.extend_from_slice(&n.to_be_bytes()[1..1 + take]);
    }
    Some(out)
}

/// A reusable bundle of app settings: a company's login, configuration sources, license server,
/// audit sink, interceptors, services and features, shipped as one crate and added in one line.
///
/// ```ignore
/// pub struct AcmeStandard { pub config: Config }
/// impl Plugin for AcmeStandard {
///     fn install(self, app: App) -> App {
///         app.config(self.config.clone())
///             .auth(ForwardAuth::new("x-forwarded-user").groups("x-forwarded-groups"))
///             .interceptor(AuditLog(AcmeAuditSink::new(&self.config)))
///             .time_policy(TimePolicy::Organization(TimeZone::named("Europe/Istanbul").unwrap()))
///     }
/// }
/// App::new("Billing").plugin(AcmeStandard { config }).feature(billing::feature())
/// ```
pub trait Plugin {
    fn install(self, app: App) -> App;
}

/// Which time zone moments are shown and read in.
#[derive(Clone, Copy, Debug)]
pub enum TimePolicy {
    /// The zone the user's browser reports, or `default` until it does.
    User { default: TimeZone },
    /// One zone for everyone, such as the organization's: an office in Istanbul shows Istanbul
    /// time even to staff connecting from abroad.
    Organization(TimeZone),
}

/// A Zeyn application.
///
/// ```ignore
/// App::new("Inventory")
///     .service(ProductStore::default())
///     .feature(stock::feature())
///     .serve("127.0.0.1:3000")
///     .await
/// ```
pub struct App {
    name: Text,
    i18n: I18n,
    services: Services,
    features: Vec<Feature>,
    auth: Option<Arc<dyn Authenticator>>,
    interceptors: Vec<Arc<dyn Interceptor>>,
    timeout: Duration,
    secure_cookies: bool,
    mcp: Mcp,
    home: Option<&'static str>,
    time: TimePolicy,
    fixed_now: Option<DateTime>,
    zone_picker: bool,
    api_prefix: String,
    config: Config,
    tenancy: Option<Arc<dyn TenantResolver>>,
    license: Option<(Arc<dyn LicenseSource>, Duration)>,
    login_url: Option<String>,
    cors: Vec<String>,
    api_docs: Option<ApiDocs>,
    theme: Option<Theme>,
    base_path: String,
    public_path: Option<String>,
    rate: Option<RateLimit>,
    write_rate: Option<RateLimit>,
    rate_store: Arc<dyn RateStore>,
    trusted: Vec<IpRange>,
    deny: Vec<IpRange>,
    allow: Vec<IpRange>,
    setup_problems: Vec<String>,
    views: Option<Arc<dyn crate::grid::DataGridViewStore>>,
    native_menu: bool,
    palette_keys: String,
    extras: ops::Extras,
    sign_in: auth::Extras,
    #[cfg(feature = "assistant")]
    assistant: Option<Arc<crate::assistant::Assistant>>,
    looks: theme::Settings,
    #[cfg(feature = "islands")]
    islands_dir: Option<std::path::PathBuf>,
    #[cfg(feature = "realtime")]
    realtime: Option<realtime::Hub>,
}

impl App {
    pub fn new(name: impl Into<Text>) -> Self {
        App {
            name: name.into(),
            i18n: I18n::new("en"),
            services: Services::default(),
            features: Vec::new(),
            auth: None,
            interceptors: Vec::new(),
            timeout: Duration::from_secs(30),
            secure_cookies: false,
            mcp: Mcp::off(),
            home: None,
            time: TimePolicy::User { default: TimeZone::UTC },
            fixed_now: None,
            zone_picker: false,
            api_prefix: "/api/v1".into(),
            config: Config::default(),
            tenancy: None,
            license: None,
            login_url: None,
            cors: Vec::new(),
            api_docs: None,
            theme: None,
            base_path: String::new(),
            public_path: None,
            rate: None,
            write_rate: None,
            rate_store: Arc::new(MemoryRateStore::default()),
            trusted: Vec::new(),
            deny: Vec::new(),
            allow: Vec::new(),
            setup_problems: Vec::new(),
            views: None,
            native_menu: true,
            palette_keys: crate::command::PALETTE_SHORTCUT.into(),
            extras: ops::Extras::default(),
            sign_in: auth::Extras::default(),
            #[cfg(feature = "assistant")]
            assistant: None,
            looks: theme::Settings::default(),
            #[cfg(feature = "islands")]
            islands_dir: None,
            #[cfg(feature = "realtime")]
            realtime: None,
        }
    }

    /// Turns on the realtime channel: collaborative documents and change topics over WebSocket
    /// (see `docs/realtime.md`). The hub is also registered as a service, so actions can take
    /// `hub: Hub` and call `hub.notify(&ctx, "topic")`.
    #[cfg(feature = "realtime")]
    pub fn realtime(mut self, realtime: realtime::Realtime) -> Self {
        let hub = realtime.install();
        self.realtime = Some(hub.clone());
        self.service(hub)
    }

    /// Serves the WASM islands that `zeyn build-islands` wrote to `dir` (usually `dist/islands`).
    /// Pages that hold an [`Island`](crate::ui::Island) get the loader and a policy that allows
    /// WebAssembly; other pages are unchanged. When `dir` has no manifest, islands show their
    /// fallback and the server logs a warning.
    #[cfg(feature = "islands")]
    pub fn islands(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.islands_dir = Some(dir.into());
        self
    }

    /// `false` stops the browser's own context menu everywhere except in text inputs, text areas
    /// and editable text. This protects nothing (any user can still view the source, use the
    /// developer tools or turn scripts off) and it takes away spell check, copy link, translate
    /// and accessibility tools. Keep the default (`true`) unless a kiosk or a strict brief asks
    /// for it; Zeyn's own menus work either way.
    pub fn browser_context_menu(mut self, allow: bool) -> Self {
        self.native_menu = allow;
        self
    }

    /// The shortcut that opens the command palette, `Alt+Shift+P` by default. Any shortcut works,
    /// also one the browser uses such as `Ctrl+Shift+P` or `F1` (choosing it here is the opt in);
    /// `App::check` refuses ones the browser never passes to the page, such as `Ctrl+T`.
    pub fn palette_shortcut(mut self, keys: &str) -> Self {
        self.palette_keys = keys.to_string();
        self
    }

    /// Saves each signed in user's grid layout, sort and filters on the server, so they come
    /// back on every device. Without a store the browser keeps them in `localStorage`.
    pub fn datagrid_views(mut self, store: impl crate::grid::DataGridViewStore) -> Self {
        self.views = Some(Arc::new(store));
        self
    }

    /// The organization's look. See [`Theme`] and `docs/theming.md`.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Serves the app under a path such as `/inventory`, like a base URL in Vue. Links, redirects,
    /// assets, runtime requests and cookies all use it, so several apps can share one host.
    pub fn base_path(mut self, path: &str) -> Self {
        self.base_path = normalize_base(path);
        self
    }

    /// For proxies that remove the prefix before forwarding (such as Traefik `StripPrefix`): the app
    /// listens at `/` but builds every URL and cookie path with this prefix.
    pub fn public_path(mut self, path: &str) -> Self {
        self.public_path = Some(normalize_base(path));
        self
    }

    /// Limit for every request except static assets.
    pub fn rate_limit(mut self, limit: RateLimit) -> Self {
        self.rate = Some(limit);
        self
    }

    /// Extra, usually stricter, limit for form submits, API calls and MCP calls.
    pub fn write_limit(mut self, limit: RateLimit) -> Self {
        self.write_rate = Some(limit);
        self
    }

    /// Where rate counters live. In memory by default (one server).
    pub fn rate_store(mut self, store: impl RateStore) -> Self {
        self.rate_store = Arc::new(store);
        self
    }

    fn ranges<I, S>(&mut self, what: &str, ranges: I) -> Vec<IpRange>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        parse_ranges(ranges).unwrap_or_else(|e| {
            self.setup_problems.push(format!("{what}: {e}"));
            Vec::new()
        })
    }

    /// Proxies and load balancers whose `X-Forwarded-For` header is believed. Without this the
    /// header is ignored, because any client can write it.
    pub fn trusted_proxies<I, S>(mut self, ranges: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.trusted = self.ranges("trusted_proxies", ranges);
        self
    }

    /// Addresses or ranges that are always refused.
    pub fn deny_ips<I, S>(mut self, ranges: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.deny = self.ranges("deny_ips", ranges);
        self
    }

    /// Only these addresses or ranges may connect, such as an internal network.
    pub fn allow_only_ips<I, S>(mut self, ranges: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allow = self.ranges("allow_only_ips", ranges);
        self
    }

    /// Publishes `{api_prefix}/openapi.json` (import it in Postman, Swagger UI, Scalar or a client
    /// generator) and a readable reference at `{api_prefix}/docs`. Off by default, because it shows
    /// every endpoint and the permission it needs.
    ///
    /// ```ignore
    /// let docs = match config.get("docs.mode") {
    ///     Some("open") => Some(ApiDocs::open()),
    ///     Some("basic") => Some(ApiDocs::basic(StaticCredentials::new("docs", config.secret("docs.password")?))),
    ///     _ => None,
    /// };
    /// ```
    pub fn api_docs(mut self, access: ApiDocs) -> Self {
        self.api_docs = Some(access);
        self
    }

    /// Lets browser apps on these exact origins (such as `https://app.example.com`) call the API
    /// with an `Authorization` header. Cookies are never sent cross origin, so screens stay
    /// protected by their CSRF token.
    pub fn cors<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cors.extend(origins.into_iter().map(Into::into));
        self
    }

    /// Configuration loaded at startup (see `Config::load`). Available as `ctx.config()` and as an
    /// injectable `Config` service.
    pub fn config(mut self, config: Config) -> Self {
        self.services.insert(config.clone());
        self.config = config;
        self
    }

    /// Finds the tenant of each request. A signed in user must belong to the resolved tenant
    /// ([`Identity::member_of`]); otherwise the request runs signed out, so public screens still
    /// work and nothing else does. Without a resolved tenant the user's own tenant applies.
    pub fn tenancy(mut self, resolver: impl TenantResolver) -> Self {
        self.tenancy = Some(Arc::new(resolver));
        self
    }

    /// Loads licenses from a source and reloads them every `refresh`. When a reload fails the last
    /// license is kept, so a short license server outage does not stop work.
    pub fn license(mut self, source: impl LicenseSource, refresh: Duration) -> Self {
        self.license = Some((Arc::new(source), refresh));
        self
    }

    /// Central login page. Users who are not signed in and open a protected page are sent there
    /// with `return_to` set to the page they wanted.
    pub fn login_url(mut self, url: impl Into<String>) -> Self {
        self.login_url = Some(url.into());
        self
    }

    /// Path prefix of API endpoints and of `openapi.json`. Default `/api/v1`.
    pub fn api_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.api_prefix = prefix.into().trim_end_matches('/').to_string();
        self
    }

    /// Shows a time zone picker in the menu so users can choose a zone by hand. Only under
    /// `TimePolicy::User`.
    pub fn time_zone_picker(mut self) -> Self {
        self.zone_picker = true;
        self
    }

    /// Time zone policy. Default: the user's browser zone, UTC until it is known.
    pub fn time_policy(mut self, policy: TimePolicy) -> Self {
        self.time = policy;
        self
    }

    /// Fixes the clock for every request. For tests and demos.
    pub fn fixed_now(mut self, now: DateTime) -> Self {
        self.fixed_now = Some(now);
        self
    }

    /// Screen that `/` redirects to. It must have a route.
    pub fn home<S: ScreenDef>(mut self, _screen: S) -> Self {
        self.home = Some(S::META.name);
        self
    }

    /// Applies a plugin. Settings made after this line override the plugin's.
    pub fn plugin(self, plugin: impl Plugin) -> Self {
        plugin.install(self)
    }

    /// Message catalogs and default language.
    pub fn i18n(mut self, i18n: I18n) -> Self {
        self.i18n = i18n;
        self
    }

    pub fn service<T: Service>(mut self, service: T) -> Self {
        self.services.insert(service);
        self
    }

    pub fn auth(mut self, auth: impl Authenticator) -> Self {
        self.auth = Some(Arc::new(auth));
        self
    }

    pub fn interceptor(mut self, interceptor: impl Interceptor) -> Self {
        self.interceptors.push(Arc::new(interceptor));
        self
    }

    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.push(feature);
        self
    }

    /// Longest time a screen, grid or form call may take. 30 seconds unless set.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Marks cookies `Secure`. Turn on whenever the app is served over HTTPS.
    pub fn secure_cookies(mut self, secure: bool) -> Self {
        self.secure_cookies = secure;
        self
    }

    /// Turns on the MCP endpoint and the WebMCP tools for outside AI agents. Off unless set; see
    /// [`Mcp`] for what agents get and why it is off by default.
    pub fn mcp(mut self, mcp: Mcp) -> Self {
        self.mcp = mcp;
        self
    }

    /// Turns on the embedded AI assistant: a chat on every page (and wherever a screen places
    /// `AssistantChat`) that answers with the app's own data and acts through the same tools as
    /// MCP, as the signed in user. See `docs/assistant.md`.
    #[cfg(feature = "assistant")]
    pub fn assistant(mut self, assistant: crate::assistant::Assistant) -> Self {
        self.assistant = Some(Arc::new(assistant));
        self
    }

    /// Old name of [`App::mcp`]; kept for one release.
    #[deprecated(since = "0.0.3", note = "renamed to `App::mcp`")]
    #[allow(deprecated)]
    pub fn agents(self, agents: Agents) -> Self {
        self.mcp(agents)
    }

    /// Checks routes, screen names, menus and that every service a screen needs is registered.
    pub fn check(&self) -> Result<(), Vec<String>> {
        let mut problems = self.setup_problems.clone();
        let mut paths: HashMap<String, &str> = HashMap::new();
        let mut names: HashMap<&str, &str> = HashMap::new();
        for f in &self.features {
            if let Err(p) = f.check() {
                problems.extend(p.into_iter().map(|p| format!("feature `{}`: {p}", f.name())));
            }
            for s in f.screens() {
                if let Some(other) = names.insert(s.meta.name, f.name()) {
                    if other != f.name() {
                        problems.push(format!(
                            "screen `{}` is registered in features `{other}` and `{}`",
                            s.meta.name,
                            f.name()
                        ));
                    }
                }
                if let Some(path) = f.path_of(&s.meta) {
                    if let Some(other) = paths.insert(path.clone(), s.meta.name) {
                        if other != s.meta.name {
                            problems.push(format!("route `{path}` is used by screens `{other}` and `{}`", s.meta.name));
                        }
                    }
                }
                for dep in (s.deps)() {
                    if !self.services.contains(dep.id) {
                        problems.push(format!(
                            "screen `{}` needs service `{}`, which is not registered\n  = help: add `.service(...)` to the App",
                            s.meta.name, dep.name
                        ));
                    }
                }
            }
        }
        for f in &self.features {
            for (i, a) in f.apis().iter().enumerate() {
                if f.apis()[..i].iter().any(|b| b.meta.name == a.meta.name) {
                    problems.push(format!("api `{}` is published twice in feature `{}`", a.meta.name, f.name()));
                }
                for dep in (a.deps)() {
                    if !self.services.contains(dep.id) {
                        problems.push(format!(
                            "api `{}` needs service `{}`, which is not registered\n  = help: add `.service(...)` to the App",
                            a.meta.name, dep.name
                        ));
                    }
                }
            }
        }
        let mut media: HashMap<&str, &str> = HashMap::new();
        for f in &self.features {
            for m in f.media_sources() {
                if let Some(other) = media.insert(m.name, f.name()) {
                    if other != f.name() {
                        problems.push(format!(
                            "media source `{}` is registered in features `{other}` and `{}`",
                            m.name,
                            f.name()
                        ));
                    }
                }
            }
        }
        if let Some(home) = self.home {
            if !self.features.iter().any(|f| f.screens().iter().any(|s| s.meta.name == home && s.meta.route.is_some()))
            {
                problems.push(format!("home screen `{home}` is not registered or has no route"));
            }
        }
        let commands: Vec<(&str, &crate::command::Command)> =
            self.features.iter().flat_map(|f| f.commands().iter().map(move |c| (f.name(), c))).collect();
        problems.extend(crate::command::problems(&commands, false, &self.palette_keys));
        match crate::command::normalize_shortcut(&self.palette_keys) {
            None => problems.push(format!(
                "the palette shortcut `{}` cannot be read
  = help: write modifiers and one key joined by `+`, such as `Alt+Shift+P`",
                self.palette_keys
            )),
            Some(k) if crate::command::browser_shortcut(&k) == Some(crate::command::BrowserShortcut::Reserved) => {
                problems.push(format!(
                    "the palette shortcut `{k}` is kept by the browser and never reaches the page
  = help: pick a free shortcut such as `Alt+Shift+P`"
                ))
            }
            _ => {}
        }
        problems.extend(self.theme_problems());
        problems.dedup();
        if problems.is_empty() { Ok(()) } else { Err(problems) }
    }

    /// Machine readable description of the app, used by `zeyn mcp` and `zeyn routes`.
    pub fn describe(&self) -> Json {
        json!({
            "name": self.i18n.text(self.i18n.default_locale(), &self.name),
            "locales": self.i18n.locales(),
            "default_locale": self.i18n.default_locale(),
            "mcp": self.mcp.is_on().then(|| json!({
                "writes": self.mcp.writes,
                "server": self.mcp.server,
                "webmcp": self.mcp.browser,
            })),
            "features": self.features.iter().map(|f| json!({
                "name": f.name(),
                "guards": f.guards().iter().map(|g| g.name()).collect::<Vec<_>>(),
                "screens": f.screens().iter().map(|s| json!({
                    "name": s.meta.name,
                    "route": s.meta.route,
                    "path": f.path_of(&s.meta),
                    "access": s.meta.access,
                    "deps": (s.deps)().iter().map(|d| d.name).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "apis": f.apis().iter().map(|a| json!({
                    "action": a.meta.name,
                    "path": format!("{}/{}/{}", self.api_prefix, f.name(), a.meta.name),
                    "access": a.meta.access,
                })).collect::<Vec<_>>(),
                "menu": f.menu_groups().iter().map(|g| json!({
                    "label": self.i18n.text(self.i18n.default_locale(), &g.label),
                    "icon": g.icon,
                    "items": g.items.iter().map(|i| json!({
                        "label": self.i18n.text(self.i18n.default_locale(), &i.label),
                        "screen": i.screen,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
                "commands": f.commands().iter().map(crate::command::Command::schema).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "missing_translations": self.i18n.missing_keys(),
            "problems": self.check().err().unwrap_or_default(),
        })
    }

    /// Builds the HTTP router. Fails when `check` finds problems.
    pub fn router(mut self) -> Result<Router, Vec<String>> {
        self.check()?;
        let docs_enabled = self.api_docs.is_some();
        let mount = self.base_path.clone();
        let url_prefix = self.public_path.clone().unwrap_or_else(|| self.base_path.clone());
        auth::resolve(&mut self.sign_in, &url_prefix, &mut self.login_url);
        let mut screens = HashMap::new();
        for (fi, f) in self.features.iter().enumerate() {
            for s in f.screens() {
                screens.insert(s.meta.name, (fi, *s));
            }
        }
        let look = self.look();
        #[cfg(feature = "islands")]
        let islands = self.islands_dir.as_deref().and_then(|dir| match islands::Islands::load(dir, &url_prefix) {
            Ok(i) => Some(i),
            Err(e) => {
                tracing::warn!("islands are not served, so they show their fallback: {e}");
                None
            }
        });
        let state = Arc::new(AppState {
            name: self.name,
            i18n: Arc::new(self.i18n),
            services: Arc::new(self.services),
            features: self.features,
            screens,
            auth: self.auth,
            interceptors: self.interceptors,
            timeout: self.timeout,
            secure_cookies: self.secure_cookies,
            mcp: self.mcp,
            time: self.time,
            fixed_now: self.fixed_now,
            zone_picker: self.zone_picker,
            api_prefix: self.api_prefix,
            config: self.config,
            tenancy: self.tenancy,
            license: self.license,
            licenses: Mutex::new(HashMap::new()),
            login_url: self.login_url,
            rate: self.rate,
            write_rate: self.write_rate,
            rate_store: self.rate_store,
            trusted: self.trusted,
            deny: self.deny,
            allow: self.allow,
            api_docs: self.api_docs,
            prefix: url_prefix,
            look,
            views: self.views,
            native_menu: self.native_menu,
            palette_keys: crate::command::normalize_shortcut(&self.palette_keys)
                .unwrap_or_else(|| crate::command::PALETTE_SHORTCUT.into()),
            extras: self.extras,
            sign_in: self.sign_in,
            #[cfg(feature = "assistant")]
            assistant: self.assistant,
            #[cfg(feature = "islands")]
            islands,
            #[cfg(feature = "realtime")]
            realtime: self.realtime,
        });
        ops::start_jobs(&state);
        let mut router = Router::new();
        for f in &state.features {
            for s in f.screens() {
                if let Some(path) = f.path_of(&s.meta) {
                    let name = s.meta.name;
                    router = router.route(
                        &path,
                        get(move |st: State<Arc<AppState>>, h: HeaderMap, q: Query<ParamsQuery>| page(st, h, q, name)),
                    );
                }
            }
        }
        for (fi, f) in state.features.iter().enumerate() {
            for (ai, a) in f.apis().iter().enumerate() {
                let path = format!("{}/{}/{}", state.api_prefix, f.name(), a.meta.name);
                router = router.route(
                    &path,
                    post(move |st: State<Arc<AppState>>, h: HeaderMap, body: axum::body::Bytes| {
                        agent::api_call(st, h, body, fi, ai)
                    }),
                );
            }
        }
        let cors = self.cors;
        if docs_enabled {
            router = router
                .route(&format!("{}/openapi.json", state.api_prefix), get(agent::openapi))
                .route(&format!("{}/docs", state.api_prefix), get(agent::api_reference));
        }
        let home = self.home.and_then(|name| {
            state
                .features
                .iter()
                .find_map(|f| f.screens().iter().find(|s| s.meta.name == name).and_then(|s| f.path_of(&s.meta)))
        });
        if let Some(path) = home.filter(|p| p != "/") {
            let target = format!("{}{path}", state.prefix);
            router = router.route("/", get(move || async move { Redirect::to(&target) }));
        }
        let router = router
            .route("/_zeyn/zeyn.js", get(|| async { asset(JS, "text/javascript; charset=utf-8") }))
            .route("/_zeyn/zeyn.css", get(|| async { asset(CSS, "text/css; charset=utf-8") }))
            .route("/_zeyn/nav.js", get(|| async { asset(NAV, "text/javascript; charset=utf-8") }))
            .route("/_zeyn/theme.css", get(theme_css))
            .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
            .route("/_zeyn/screen/{screen}", get(screen_fragment))
            .route("/_zeyn/route/{screen}", get(route_redirect))
            .route("/_zeyn/menu", post(menu_call))
            .route("/_zeyn/form", post(form_call))
            .route("/_zeyn/locale", post(set_locale))
            .route("/_zeyn/timezone", post(set_time_zone));
        #[cfg(feature = "assistant")]
        let router = assistant::routes(router);
        let router = auth::routes(router, &state);
        #[cfg(feature = "accounts")]
        let router = accounts::routes(router, &state);
        #[cfg(feature = "islands")]
        let router = islands::routes(router);
        #[cfg(feature = "realtime")]
        let router = realtime::routes(router);
        #[cfg(feature = "pdf")]
        let router = pdf::routes(router);
        let router = theme::routes(pro::routes(media::routes(ui::routes(session::routes(data::routes(ops::routes(
            agent::routes(grid::routes(router)),
        )))))))
        .fallback(not_found);
        #[cfg(feature = "accounts")]
        let router = if state.sign_in.accounts.is_some() {
            router.layer(axum::middleware::from_fn_with_state(state.clone(), accounts::layer))
        } else {
            router
        };
        let router = auth::layers(router, &state);
        let router = router
            .layer(axum::middleware::from_fn_with_state(state.clone(), guard::guard_request))
            .layer(axum::middleware::map_response(guard::security_headers))
            .layer(axum::middleware::map_response(ops::stamp))
            .layer(axum::middleware::from_fn(guard::trace_request))
            .with_state(state);
        let router = if mount.is_empty() { router } else { Router::new().nest(&mount, router) };
        if cors.is_empty() {
            return Ok(router);
        }
        let origins: Vec<HeaderValue> = cors.iter().filter_map(|o| HeaderValue::from_str(o).ok()).collect();
        let layer = tower_http::cors::CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT_LANGUAGE])
            .max_age(Duration::from_secs(600));
        Ok(router.layer(layer))
    }

    /// Checks the app and serves it. With `ZEYN_DESCRIBE=1` it prints `describe()` and exits instead.
    pub async fn serve(self, addr: &str) -> std::io::Result<()> {
        if std::env::var_os("ZEYN_DESCRIBE").is_some() {
            println!("{}", serde_json::to_string_pretty(&self.describe()).unwrap_or_default());
            return Ok(());
        }
        let flush = self.start_telemetry();
        let router = self.router().map_err(|problems| {
            for p in &problems {
                tracing::error!("{p}");
            }
            std::io::Error::other("the app has problems; see the errors above")
        })?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("Zeyn is listening on http://{}", listener.local_addr()?);
        let served = axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>()).await;
        if let Some(f) = flush {
            f.shutdown().await;
        }
        served
    }

    /// Installs the default log and trace subscriber (see `server::telemetry`) and logs the
    /// startup warnings.
    fn start_telemetry(&self) -> Option<telemetry::Flush> {
        let flush = telemetry::init(&self.config, &self.i18n.text(self.i18n.default_locale(), &self.name));
        for (locale, key) in self.i18n.missing_keys() {
            tracing::warn!("translation `{key}` is missing in `{locale}`");
        }
        #[cfg(debug_assertions)]
        if let Some(d) = self.extras.latency {
            tracing::warn!("every call is delayed by {} ms (dev_latency)", d.as_millis());
        }
        if self.auth.as_ref().is_some_and(|a| a.is_development()) {
            tracing::warn!("DevAuth is on; every request has full access. Do not use it in production.");
        }
        flush
    }
}

struct AppState {
    name: Text,
    i18n: Arc<I18n>,
    services: Arc<Services>,
    features: Vec<Feature>,
    screens: HashMap<&'static str, (usize, ScreenEntry)>,
    auth: Option<Arc<dyn Authenticator>>,
    interceptors: Vec<Arc<dyn Interceptor>>,
    timeout: Duration,
    secure_cookies: bool,
    mcp: Mcp,
    time: TimePolicy,
    fixed_now: Option<DateTime>,
    zone_picker: bool,
    api_prefix: String,
    config: Config,
    tenancy: Option<Arc<dyn TenantResolver>>,
    license: Option<(Arc<dyn LicenseSource>, Duration)>,
    licenses: Mutex<HashMap<String, (Instant, Option<Arc<License>>)>>,
    login_url: Option<String>,
    rate: Option<RateLimit>,
    write_rate: Option<RateLimit>,
    rate_store: Arc<dyn RateStore>,
    trusted: Vec<IpRange>,
    deny: Vec<IpRange>,
    allow: Vec<IpRange>,
    api_docs: Option<ApiDocs>,
    prefix: String,
    look: theme::Look,
    views: Option<Arc<dyn crate::grid::DataGridViewStore>>,
    native_menu: bool,
    palette_keys: String,
    extras: ops::Extras,
    sign_in: auth::Extras,
    #[cfg(feature = "assistant")]
    assistant: Option<Arc<crate::assistant::Assistant>>,
    #[cfg(feature = "islands")]
    islands: Option<islands::Islands>,
    #[cfg(feature = "realtime")]
    realtime: Option<realtime::Hub>,
}

/// Decodes `%XX` sequences in a cookie value.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Failure of a call: an application error, a timeout, or a refusal with its own status and code
/// that the runtime acts on (such as `reason_required`).
enum Failure {
    App(Error),
    Timeout,
    Status(StatusCode, &'static str, Text),
}

impl From<Error> for Failure {
    /// A refusal for a missing second factor becomes `mfa_required`, which starts a step up sign in.
    fn from(e: Error) -> Self {
        match e {
            Error::Denied(Text::Key { ref key, .. }) if key == crate::auth::MFA_REQUIRED => {
                Failure::Status(StatusCode::UNAUTHORIZED, "mfa_required", Text::key(crate::auth::MFA_REQUIRED))
            }
            e => Failure::App(e),
        }
    }
}

#[derive(Deserialize)]
struct ParamsQuery {
    params: Option<String>,
    /// `?return_to=` as sign in redirects and the session runtime send it; it becomes the
    /// params `{"return_to": ...}` of a page opened without `params`.
    return_to: Option<String>,
}

impl ParamsQuery {
    fn value(&self) -> Result<Json, Failure> {
        match &self.params {
            None if self.return_to.is_some() => Ok(json!({ "return_to": self.return_to })),
            None => Ok(Json::Null),
            Some(p) => serde_json::from_str(p).map_err(|_| Failure::App(Error::NotFound)),
        }
    }
}

pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// Compares two strings in time that does not depend on where they differ.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Compares two byte strings in time that does not depend on where they differ.
#[cfg(feature = "accounts")]
pub(crate) fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("the operating system random source must work");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_token(t: &str) -> bool {
    t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Most tenants whose license is kept in memory. The tenant may come from a header or the host
/// name, so a client can make ids up; when the cache is full, stale entries go first and, if every
/// entry is fresh, the cache starts over.
const MAX_LICENSES: usize = 4096;

impl AppState {
    /// URL of an app path, with the base path in front.
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.prefix)
    }

    /// Cookie `Path` attribute for this app.
    fn cookie_path(&self) -> &str {
        if self.prefix.is_empty() { "/" } else { &self.prefix }
    }

    fn ctx(&self, headers: &HeaderMap) -> Ctx {
        let mut identity = self.auth.as_ref().and_then(|a| a.authenticate(headers)).unwrap_or_default();
        auth::adjust(self, headers, &mut identity);
        let accept = headers.get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok());
        let locale = self.i18n.negotiate(cookie(headers, "zeyn_locale"), accept);
        let tz = match self.time {
            TimePolicy::Organization(zone) => zone,
            TimePolicy::User { default } => identity
                .time_zone
                .as_deref()
                .and_then(TimeZone::named)
                .or_else(|| cookie(headers, "zeyn_tz_manual").and_then(|z| TimeZone::named(&percent_decode(z))))
                .or_else(|| cookie(headers, "zeyn_tz").and_then(|z| TimeZone::named(&percent_decode(z))))
                .unwrap_or(default),
        };
        let resolved = self.tenancy.as_ref().and_then(|r| r.resolve(headers)).and_then(|t| Tenant::new(&t));
        let tenant = match resolved {
            Some(t) if identity.is_signed_in() && !identity.member_of(t.id()) => {
                tracing::debug!(tenant = t.id(), "the user is not a member of the request's tenant; signed out");
                identity = Identity::anonymous();
                Some(t)
            }
            Some(t) => Some(t),
            None => identity.tenant.as_deref().and_then(Tenant::new),
        };
        let ctx = Ctx::for_request(
            identity,
            locale.into(),
            self.services.clone(),
            self.i18n.clone(),
            tz,
            tenant,
            self.config.clone(),
            self.look.pick(cookie(headers, "zeyn_theme")),
            self.extras.session.reauth_within,
        );
        data::read_reason(&ctx, headers);
        match self.fixed_now {
            Some(now) => ctx.with_now(now),
            None => ctx,
        }
    }

    /// Context with the license that applies to the request, loading or refreshing it when due.
    async fn licensed(&self, ctx: Ctx) -> Ctx {
        let Some((source, refresh)) = &self.license else { return ctx };
        let key = ctx.tenant().map(|t| t.id().to_string()).unwrap_or_default();
        let cached = self.licenses.lock().ok().and_then(|c| c.get(&key).cloned());
        if let Some((at, license)) = &cached {
            if at.elapsed() < *refresh {
                return ctx.with_license(license.clone());
            }
        }
        let license = match source.load(ctx.tenant()).await {
            Ok(l) => l.map(Arc::new),
            Err(e) => {
                tracing::warn!("license source failed, keeping the last license: {e}");
                cached.and_then(|(_, l)| l)
            }
        };
        if let Ok(mut c) = self.licenses.lock() {
            if c.len() >= MAX_LICENSES && !c.contains_key(&key) {
                c.retain(|_, (at, _)| at.elapsed() < *refresh);
                if c.len() >= MAX_LICENSES {
                    c.clear();
                }
            }
            c.insert(key, (Instant::now(), license.clone()));
        }
        ctx.with_license(license)
    }

    /// Existing CSRF token from the cookie, or a new one with the cookie to set.
    fn csrf(&self, headers: &HeaderMap) -> (String, Option<String>) {
        match cookie(headers, "zeyn_csrf").filter(|t| valid_token(t)) {
            Some(t) => (t.to_string(), None),
            None => {
                let t = new_token();
                let secure = if self.secure_cookies { "; Secure" } else { "" };
                let c = format!("zeyn_csrf={t}; Path={}; HttpOnly; SameSite=Strict{secure}", self.cookie_path());
                (t, Some(c))
            }
        }
    }

    fn check_csrf(&self, headers: &HeaderMap) -> Result<(), Failure> {
        let header = headers.get("x-zeyn-csrf").and_then(|v| v.to_str().ok()).unwrap_or("");
        match cookie(headers, "zeyn_csrf") {
            Some(c) if valid_token(c) && same(c, header) => Ok(()),
            _ => Err(Failure::App(Error::Denied(Text::key("zeyn.error.csrf")))),
        }
    }

    /// Runs application code (an action, a data source, a screen builder) with the app's time
    /// limit, inside a `run` span.
    async fn timed<T>(&self, fut: impl Future<Output = crate::Result<T>>) -> Result<T, Failure> {
        use tracing::Instrument;
        match tokio::time::timeout(self.timeout, fut).instrument(tracing::info_span!("run")).await {
            Ok(r) => r.map_err(Failure::App),
            Err(_) => Err(Failure::Timeout),
        }
    }

    /// Finds, checks and builds a screen: access, then feature guards.
    async fn open(&self, ctx: &Ctx, name: &str, params: Json) -> Result<(&Feature, ScreenEntry, Screen), Failure> {
        let (fi, entry) = *self.screens.get(name).ok_or(Failure::App(Error::NotFound))?;
        let feature = &self.features[fi];
        if !ctx.identity().can(&entry.meta.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        feature.check_guards(ctx)?;
        data::check_reason(ctx, &entry.meta)?;
        let only_return_to = params.as_object().is_some_and(|o| o.len() == 1 && o.contains_key("return_to"));
        let screen = match self.timed((entry.build)(ctx.clone(), params)).await {
            Err(_) if only_return_to => self.timed((entry.build)(ctx.clone(), Json::Null)).await?,
            other => other?,
        };
        Ok((feature, entry, screen))
    }

    fn before(&self, ctx: &Ctx, call: &Call) -> Result<(), Failure> {
        let span = tracing::Span::current();
        if !span.is_disabled() {
            span.record("kind", tracing::field::debug(call.kind));
            span.record("screen", call.screen);
            if let Some(a) = &call.action {
                span.record("action", a.name);
            }
            if let Some(user) = ctx.identity().user.as_deref() {
                span.record("user", user);
            }
            if let Some(tenant) = ctx.tenant() {
                span.record("tenant", tenant.id());
            }
        }
        session::check_reauth(self, ctx, call)?;
        for i in &self.interceptors {
            let _span = tracing::debug_span!("interceptor", name = i.name(), stage = "before").entered();
            i.before(ctx, call)?;
        }
        Ok(())
    }

    #[cfg_attr(not(feature = "realtime"), allow(dead_code))]
    /// Runs the interceptors for a call that runs no application code itself, such as a socket
    /// handshake: `before`, then `after` with its outcome.
    fn admit_call(&self, ctx: &Ctx, call: &Call) -> Result<(), Failure> {
        let started = Instant::now();
        let checked = self.before(ctx, call);
        self.after(ctx, call, started, &checked);
        checked
    }

    /// Runs `work` between the interceptors' `before` and `after`, so no endpoint can run
    /// application code without them: `before` records the call on the span, checks
    /// re-authentication and runs every interceptor; `work` starts only when they all pass;
    /// `after` sees the outcome, also when `before` refused the call. `work` is a closure so that
    /// nothing it builds (form validation, an action's input) runs before the interceptors.
    async fn intercept<T, F>(&self, ctx: &Ctx, call: &Call, work: impl FnOnce() -> F) -> Result<T, Failure>
    where
        F: Future<Output = Result<T, Failure>>,
    {
        let started = Instant::now();
        let result = async {
            self.before(ctx, call)?;
            work().await
        }
        .await;
        self.after(ctx, call, started, &result);
        result
    }

    fn after<T>(&self, ctx: &Ctx, call: &Call, started: Instant, result: &Result<T, Failure>) {
        let outcome = CallOutcome {
            ok: result.is_ok(),
            error: match result {
                Ok(_) => None,
                Err(Failure::Timeout) => Some("timeout"),
                Err(Failure::App(e)) => Some(e.code()),
                Err(Failure::Status(_, code, _)) => Some(*code),
            },
            duration: started.elapsed(),
        };
        let span = tracing::Span::current();
        if !span.is_disabled() {
            span.record("ok", outcome.ok);
            span.record("error", outcome.error);
        }
        tracing::debug!(
            kind = ?call.kind,
            screen = call.screen,
            action = call.action.as_ref().map(|a| a.name),
            user = ctx.identity().user.as_deref(),
            tenant = ctx.tenant().map(|t| t.id()),
            ok = outcome.ok,
            error = outcome.error,
            ms = outcome.duration.as_millis() as u64,
            "call finished"
        );
        for i in &self.interceptors {
            let _span = tracing::debug_span!("interceptor", name = i.name(), stage = "after").entered();
            i.after(ctx, call, &outcome);
        }
    }

    /// Key of a saved grid view: tenant, screen and node, so tenants never share views.
    fn view_key(ctx: &Ctx, screen: &str, node: &NodePath) -> String {
        format!("{}:{screen}#{node}", ctx.tenant().map(|t| t.id().to_string()).unwrap_or_default())
    }

    /// The signed in user's saved view of a grid, cleaned by the grid. `None` without a store,
    /// without a user or when nothing is saved; a failing store is logged and ignored.
    async fn saved_view(
        &self,
        ctx: &Ctx,
        grid: &dyn GridSource,
        screen: &str,
        node: &NodePath,
    ) -> Option<DataGridView> {
        let store = self.views.as_ref()?;
        let user = ctx.identity().user.clone()?;
        match self.timed(store.load(&user, &Self::view_key(ctx, screen, node))).await {
            Ok(view) => view.map(|v| grid.clean_view(v)),
            Err(_) => {
                tracing::warn!("grid view store failed to load a view");
                None
            }
        }
    }

    /// Loads the first page of every grid the user may read, in the user's saved view.
    async fn preload(
        &self,
        ctx: &Ctx,
        nodes: Vec<(NodePath, &crate::screen::Node)>,
        name: &str,
    ) -> HashMap<String, LoadedPage> {
        let mut loaded = HashMap::new();
        let saved = self.views.is_some() && ctx.identity().is_signed_in();
        for (i, node) in nodes {
            let Some(grid) = node.grid() else { continue };
            if !ctx.identity().can(&grid.action().access) {
                continue;
            }
            let view = self.saved_view(ctx, grid, name, &i).await;
            if let Ok(mut page) =
                self.timed(grid.fetch_at(ctx.clone(), grid.first_query(view.as_ref()), Vec::new())).await
            {
                page.view = view;
                page.saved_views = saved;
                loaded.insert(i.to_string(), page);
            }
        }
        loaded
    }

    fn error_response(&self, ctx: &Ctx, failure: Failure) -> Response {
        let (status, code, message) = match &failure {
            Failure::Timeout => (StatusCode::GATEWAY_TIMEOUT, "timeout", Text::key("zeyn.error.timeout")),
            Failure::Status(status, code, message) => (*status, *code, message.clone()),
            Failure::App(e) => {
                let status = match e {
                    Error::Forbidden | Error::Denied(_) => StatusCode::FORBIDDEN,
                    Error::NotFound => StatusCode::NOT_FOUND,
                    Error::Conflict => StatusCode::CONFLICT,
                    Error::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
                    Error::Internal(detail) => {
                        tracing::error!("{detail}");
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                };
                (status, e.code(), e.message())
            }
        };
        if let Failure::App(Error::Validation(errors)) = failure {
            let list: Vec<Json> = errors
                .errors
                .iter()
                .map(|e| json!({ "field": e.field, "rule": e.rule, "message": ctx.t(&e.message) }))
                .collect();
            return (status, axum::Json(json!({ "error": code, "errors": list }))).into_response();
        }
        (status, axum::Json(json!({ "error": code, "message": ctx.t(&message) }))).into_response()
    }

    fn error_page(&self, ctx: &Ctx, failure: Failure, csrf: &str) -> Response {
        let (status, message) = match failure {
            Failure::Timeout => (StatusCode::GATEWAY_TIMEOUT, Text::key("zeyn.error.timeout")),
            Failure::Status(status, _, message) => (status, message),
            Failure::App(e) => {
                let status = match &e {
                    Error::Forbidden | Error::Denied(_) => StatusCode::FORBIDDEN,
                    Error::NotFound => StatusCode::NOT_FOUND,
                    Error::Conflict => StatusCode::CONFLICT,
                    Error::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
                    Error::Internal(detail) => {
                        tracing::error!("{detail}");
                        StatusCode::INTERNAL_SERVER_ERROR
                    }
                };
                (status, e.message())
            }
        };
        let body = format!(
            r#"<section class="z-screen z-error" aria-labelledby="z-error-title"><h1 id="z-error-title">{}</h1></section>"#,
            escape(&ctx.t(&message))
        );
        (status, Html(self.shell(ctx, csrf, &ctx.t(&message), &body, None))).into_response()
    }

    /// Full page around a screen: menu, language selector, toasts and assets.
    fn shell(&self, ctx: &Ctx, csrf: &str, title: &str, main: &str, current: Option<&str>) -> String {
        let r = Render::new("shell", ctx.locale(), ctx.i18n()).with_tz(ctx.time_zone()).with_identity(ctx.identity());
        let mut nav = String::new();
        for f in &self.features {
            if f.check_guards(ctx).is_err() {
                continue;
            }
            for g in f.menu_groups() {
                let items: Vec<String> = g
                    .items
                    .iter()
                    .filter_map(|item| {
                        let entry = f.screens().iter().find(|s| s.meta.name == item.screen)?;
                        if !ctx.identity().can(&entry.meta.access) {
                            return None;
                        }
                        let path = f.path_of(&entry.meta)?;
                        let aria = if current == Some(item.screen) { r#" aria-current="page""# } else { "" };
                        Some(format!(
                            r#"<li><a href="{}"{aria}>{}</a></li>"#,
                            escape(&self.url(&path)),
                            r.text(&item.label)
                        ))
                    })
                    .collect();
                if !items.is_empty() {
                    nav.push_str(&format!(
                        r#"<div class="z-nav-group"><h2>{}{}</h2><ul>{}</ul></div>"#,
                        g.icon.html(),
                        r.text(&g.label),
                        items.join("")
                    ));
                }
            }
        }
        let locales = self.i18n.locales();
        if locales.len() > 1 {
            let options: String = locales
                .iter()
                .map(|l| {
                    let selected = if l == ctx.locale() { " selected" } else { "" };
                    let name = self.i18n.lookup(l, "zeyn.locale.name").unwrap_or(l);
                    format!(r#"<option value="{}"{selected}>{}</option>"#, escape(l), escape(name))
                })
                .collect();
            nav.push_str(&format!(
                r#"<div class="z-locale"><span id="z-locale-label">{}</span><div class="z-select" data-text-none=""><select id="z-locale" data-zeyn-locale>{options}</select></div></div>"#,
                r.text(&Text::key("zeyn.shell.language"))
            ));
        }
        let theme = ctx.theme().unwrap_or("system");
        nav.push_str(&self.look.picker(&r, theme));
        if self.zone_picker && matches!(self.time, TimePolicy::User { .. }) {
            let current = ctx.time_zone().name();
            let auto = r.text(&Text::key("zeyn.shell.time_zone_auto"));
            let mut options = format!(r#"<option value="">{auto}</option>"#);
            let listed = TimeZone::names().any(|n| n == current);
            for name in TimeZone::names().chain((!listed).then_some(current)) {
                let selected = if name == current { " selected" } else { "" };
                options.push_str(&format!(r#"<option value="{name}"{selected}>{}</option>"#, name.replace('_', " ")));
            }
            nav.push_str(&format!(
                r#"<div class="z-locale"><span id="z-timezone-label">{}</span><div class="z-select" data-text-none="{auto}"><select id="z-timezone" data-zeyn-timezone>{options}</select></div></div>"#,
                r.text(&Text::key("zeyn.shell.time_zone"))
            ));
        }
        let (theme_attrs, scheme) = self.look.html(theme);
        let app = r.text(&self.name);
        let (palette, commands) = pro::shell_parts(self, ctx, &r);
        #[cfg(feature = "assistant")]
        let (ai_head, ai_launcher) =
            (assistant::head(self, ctx, &escape(&self.prefix)), assistant::launcher(self, ctx, &r));
        #[cfg(not(feature = "assistant"))]
        let (ai_head, ai_launcher) = (String::new(), String::new());
        format!(
            r##"<!doctype html><html lang="{lang}"{html_attrs}><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="color-scheme" content="{scheme}"><meta name="zeyn-csrf" content="{csrf}"><meta name="zeyn-tz" content="{tz}"><meta name="zeyn-base" content="{base}"><title>{title} · {app}</title><link rel="stylesheet" href="{base}/_zeyn/zeyn.css"><script type="module" src="{base}/_zeyn/zeyn.js"></script><script type="module" src="{base}/_zeyn/nav.js"></script>{components}{theme_link}{extra}</head><body><a class="z-skip" href="#z-main">{skip}</a><div class="z-app"><nav class="z-nav" aria-label="{menu}"><p class="z-app-name">{app}</p>{palette}{nav}</nav><main class="z-main" id="z-main">{main}</main></div><div class="z-toasts" role="status" aria-live="polite"></div>{commands}{ai_launcher}</body></html>"##,
            lang = escape(ctx.locale()),
            tz = escape(ctx.time_zone().name()),
            base = escape(&self.prefix),
            components = format!(
                "{}{}{}{ai_head}",
                ui::head(&escape(&self.prefix)),
                media::head(&escape(&self.prefix)),
                pro::head(&escape(&self.prefix))
            ),
            html_attrs =
                format!("{theme_attrs}{}", if self.native_menu { "" } else { r#" data-zeyn-native-menu="off""# }),
            theme_link = if self.look.css.is_empty() {
                String::new()
            } else {
                format!(r#"<link rel="stylesheet" href="{}/_zeyn/theme.css">"#, escape(&self.prefix))
            },
            title = escape(title),
            extra = ops::head(self, ctx),
            skip = r.text(&Text::key("zeyn.shell.skip")),
            menu = r.text(&Text::key("zeyn.shell.menu")),
        )
    }
}

/// `inventory/`, `/inventory` and `/inventory/` all become `/inventory`; `/` and empty become empty.
/// Renders for the request's user: their language, time zone and permissions, with masked fields
/// masked and reads of sensitive rows recorded for the audit.
fn user_render<'a>(scope: &'a str, ctx: &'a Ctx) -> Render<'a> {
    Render::new(scope, ctx.locale(), ctx.i18n()).with_tz(ctx.time_zone()).with_identity(ctx.identity()).with_ctx(ctx)
}

fn normalize_base(path: &str) -> String {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() { String::new() } else { format!("/{trimmed}") }
}

fn asset(body: &'static str, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

async fn theme_css(State(st): State<Arc<AppState>>) -> Response {
    let css = st.look.css.clone();
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], css).into_response()
}

fn with_cookie(mut response: Response, cookie: Option<String>) -> Response {
    if let Some(c) = cookie.and_then(|c| HeaderValue::from_str(&c).ok()) {
        response.headers_mut().append(header::SET_COOKIE, c);
    }
    response
}

async fn page(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<ParamsQuery>,
    name: &'static str,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let (ctx, theme_cookie) = theme::remember(&st, ctx, &headers).await;
    let (csrf, set) = st.csrf(&headers);
    let started = Instant::now();
    let call = Call { kind: CallKind::Page, screen: name, action: None, via_agent: false };
    let result = async {
        let params = q.value()?;
        st.before(&ctx, &call)?;
        let (_, entry, screen) = st.open(&ctx, name, params.clone()).await?;
        data::note_screen_read(&ctx, &entry.meta, &params);
        let loaded = st.preload(&ctx, screen.walk(), name).await;
        let r = user_render(name, &ctx);
        let html = screen.render(&r, &entry.meta, &params, &loaded);
        Ok::<_, Failure>((ctx.t(screen.title()), html))
    }
    .await;
    st.after(&ctx, &call, started, &result);
    let partial = headers.get("x-zeyn-partial").is_some_and(|v| v == "1");
    let response = match result {
        Ok((title, html)) if partial => {
            let full = format!("{title} · {}", ctx.t(&st.name));
            #[cfg(feature = "islands")]
            let holds_island = islands::holds(&st, &html);
            #[cfg(feature = "realtime")]
            let holds_live = realtime::holds(&st, &html);
            let mut r = Html(html).into_response();
            let h = r.headers_mut();
            h.insert("x-zeyn-partial", HeaderValue::from_static("1"));
            if let Ok(v) = HeaderValue::from_str(&encode(&full)) {
                h.insert("x-zeyn-title", v);
            }
            h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            h.insert(header::VARY, HeaderValue::from_static("x-zeyn-partial"));
            #[cfg(feature = "islands")]
            if holds_island {
                h.insert("x-zeyn-islands", HeaderValue::from_static("1"));
            }
            #[cfg(feature = "realtime")]
            if holds_live {
                h.insert("x-zeyn-live", HeaderValue::from_static("1"));
            }
            r
        }
        Ok((title, html)) => {
            #[allow(unused_mut)]
            let mut page = st.shell(&ctx, &csrf, &title, &html, Some(name));
            #[cfg(feature = "realtime")]
            realtime::head(&st, &html, &mut page);
            #[cfg(feature = "islands")]
            let page = islands::page(&st, page, &html);
            #[cfg(not(feature = "islands"))]
            let page = Html(page).into_response();
            page
        }
        Err(Failure::App(Error::Forbidden | Error::Denied(_)))
            if !ctx.identity().is_signed_in() && st.login_url.is_some() =>
        {
            let here = st
                .screens
                .get(name)
                .and_then(|(fi, e)| st.features[*fi].path_of(&e.meta))
                .map(|p| st.url(&p))
                .unwrap_or_default();
            let target = st.login_url.clone().unwrap_or_default();
            let sep = if target.contains('?') { '&' } else { '?' };
            Redirect::to(&format!("{target}{sep}return_to={}", encode(&here))).into_response()
        }
        Err(Failure::Status(_, "mfa_required", _)) if st.sign_in.mfa_url.is_some() => {
            let here = st
                .screens
                .get(name)
                .and_then(|(fi, e)| st.features[*fi].path_of(&e.meta))
                .map(|p| st.url(&p))
                .unwrap_or_default();
            let target = st.sign_in.mfa_url.clone().unwrap_or_default();
            let sep = if target.contains('?') { '&' } else { '?' };
            Redirect::to(&format!("{target}{sep}return_to={}", encode(&here))).into_response()
        }
        Err(Failure::Status(_, "reason_required", _)) => data::reason_page(&st, &ctx, name, &csrf),
        Err(f) => st.error_page(&ctx, f, &csrf),
    };
    with_cookie(with_cookie(response, set), theme_cookie)
}

async fn screen_fragment(
    State(st): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Query(q): Query<ParamsQuery>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let Some((&name, _)) = st.screens.get_key_value(name.as_str()) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let call = Call { kind: CallKind::Screen, screen: name, action: None, via_agent: false };
    let started = Instant::now();
    let result = async {
        let params = q.value()?;
        st.before(&ctx, &call)?;
        let (_, entry, screen) = st.open(&ctx, name, params.clone()).await?;
        data::note_screen_read(&ctx, &entry.meta, &params);
        let loaded = st.preload(&ctx, screen.walk(), entry.meta.name).await;
        let r = user_render(entry.meta.name, &ctx);
        Ok::<_, Failure>(screen.render(&r, &entry.meta, &params, &loaded))
    }
    .await;
    st.after(&ctx, &call, started, &result);
    match result {
        Ok(html) => Html(html).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

async fn route_redirect(
    State(st): State<Arc<AppState>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Query(q): Query<ParamsQuery>,
) -> Response {
    let ctx = st.ctx(&headers);
    let Some((fi, entry)) = st.screens.get(name.as_str()) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let Some(path) = st.features[*fi].path_of(&entry.meta) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let path = st.url(&path);
    let target = match &q.params {
        Some(p) => format!("{path}?params={}", encode(p)),
        None => path,
    };
    Redirect::to(&target).into_response()
}

/// Percent encodes a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Body of a streamed export: chunks from a channel.
pub(super) struct Chunks(pub(super) tokio::sync::mpsc::Receiver<Result<axum::body::Bytes, std::io::Error>>);

impl futures_core::Stream for Chunks {
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

#[derive(Deserialize)]
struct MenuCall {
    screen: String,
    #[serde(default)]
    params: Json,
    /// Path of the item in the screen's context menu, such as `"2.1"`.
    item: String,
}

#[derive(Deserialize)]
struct FormCall {
    screen: String,
    #[serde(default)]
    params: Json,
    node: NodePath,
    #[serde(default)]
    values: Map<String, Json>,
    #[serde(default)]
    via_agent: bool,
    /// Sent by the WebMCP runtime after the user confirmed a `confirm` action in the page.
    #[serde(default)]
    confirmed: bool,
}

impl AppState {
    /// Runs an action item of a screen's context menu: the screen is built again with its params,
    /// the item is taken from that menu, its access is checked and interceptors run.
    async fn run_menu(&self, ctx: &Ctx, call: MenuCall) -> Result<Json, Failure> {
        if call.item.len() > 32 {
            return Err(Failure::App(Error::NotFound));
        }
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let item = screen.menu_call(&call.item).ok_or(Failure::App(Error::NotFound))?;
        if !ctx.identity().can(&item.meta.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        let info = Call {
            kind: CallKind::Screen,
            screen: entry.meta.name,
            action: Some(item.meta),
            via_agent: ctx.via_agent(),
        };
        let result = self.intercept(ctx, &info, || self.timed((item.run)(ctx.clone()))).await;
        result?;
        Ok(json!({ "effects": [] }))
    }

    async fn run_form(&self, ctx: &Ctx, call: FormCall) -> Result<Json, Failure> {
        let (_, entry, screen) = self.open(ctx, &call.screen, call.params).await?;
        let form = screen.node(&call.node).and_then(|n| n.form()).ok_or(Failure::App(Error::NotFound))?;
        let action = form.action();
        if !ctx.identity().can(&action.access) {
            return Err(Failure::App(Error::Forbidden));
        }
        if ctx.via_agent() {
            agent::agent_policy(&entry.meta.ai, &action, call.confirmed)?;
        }
        let info =
            Call { kind: CallKind::Form, screen: entry.meta.name, action: Some(action), via_agent: ctx.via_agent() };
        let result = self.intercept(ctx, &info, || self.timed(form.submit(ctx.clone(), call.values))).await;
        let effects = result?;
        let r = user_render(entry.meta.name, ctx);
        Ok(json!({ "effects": effects.iter().map(|e| e.to_json(&r)).collect::<Vec<_>>() }))
    }
}

async fn menu_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<MenuCall>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    match st.run_menu(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

async fn form_call(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<FormCall>,
) -> Response {
    let mut ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    if call.via_agent {
        if !(st.mcp.browser && st.mcp.writes) {
            return st.error_response(&ctx, Failure::App(Error::Forbidden));
        }
        ctx = ctx.as_agent();
    }
    match st.run_form(&ctx, call).await {
        Ok(body) => axum::Json(body).into_response(),
        Err(f) => st.error_response(&ctx, f),
    }
}

#[derive(Deserialize)]
struct LocaleCall {
    locale: String,
}

async fn set_locale(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<LocaleCall>,
) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    if !st.i18n.locales().iter().any(|l| *l == call.locale) {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    }
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    let c = format!("zeyn_locale={}; Path={}; Max-Age=31536000; SameSite=Lax{secure}", call.locale, st.cookie_path());
    with_cookie(StatusCode::NO_CONTENT.into_response(), Some(c))
}

/// RFC 9457 problem details response.
fn problem(ctx: &Ctx, failure: Failure) -> Response {
    let (status, code, message) = match &failure {
        Failure::Timeout => (StatusCode::GATEWAY_TIMEOUT, "timeout", Text::key("zeyn.error.timeout")),
        Failure::Status(status, code, message) => (*status, *code, message.clone()),
        Failure::App(e) => (
            match e {
                Error::Forbidden | Error::Denied(_) => StatusCode::FORBIDDEN,
                Error::NotFound => StatusCode::NOT_FOUND,
                Error::Conflict => StatusCode::CONFLICT,
                Error::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
                Error::Internal(detail) => {
                    tracing::error!("{detail}");
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            },
            e.code(),
            e.message(),
        ),
    };
    let mut body = json!({
        "type": format!("urn:zeyn:problem:{code}"),
        "title": status.canonical_reason().unwrap_or(code),
        "status": status.as_u16(),
        "detail": ctx.t(&message),
    });
    if let Failure::App(Error::Validation(errors)) = failure {
        body["errors"] = errors
            .errors
            .iter()
            .map(|e| json!({ "field": e.field, "rule": e.rule, "message": ctx.t(&e.message) }))
            .collect();
    }
    let mut response = (status, body.to_string()).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
    response
}

#[derive(Deserialize)]
struct ZoneCall {
    /// Empty clears the manual choice and returns to the browser's zone.
    zone: String,
}

async fn set_time_zone(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<ZoneCall>,
) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    if call.zone.is_empty() {
        let c = format!("zeyn_tz_manual=; Path={}; Max-Age=0; SameSite=Lax{secure}", st.cookie_path());
        return with_cookie(StatusCode::NO_CONTENT.into_response(), Some(c));
    }
    let Some(zone) = TimeZone::named(&call.zone) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let c = format!(
        "zeyn_tz_manual={}; Path={}; Max-Age=31536000; SameSite=Lax{secure}",
        encode(zone.name()),
        st.cookie_path()
    );
    with_cookie(StatusCode::NO_CONTENT.into_response(), Some(c))
}

async fn not_found(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let ctx = st.ctx(&headers);
    let (csrf, set) = st.csrf(&headers);
    with_cookie(st.error_page(&ctx, Failure::App(Error::NotFound), &csrf), set)
}
