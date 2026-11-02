use crate::action::{Access, Error, Result};
use crate::i18n::{I18n, Text};
use crate::screen::Screen;
use crate::platform::{Config, License, Tenant};
use crate::time::{Date, DateTime, TimeZone};
use std::any::{Any, TypeId};
use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// Boxed future returned by generated `call` and `build` functions.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// Who is making the request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    pub user: Option<String>,
    pub permissions: BTreeSet<String>,
    /// Grants every permission. Only for development and tests.
    pub superuser: bool,
    /// Tenant named by the identity provider. Wins over the tenant resolved from the request.
    pub tenant: Option<String>,
    /// Time zone from the user's profile, set by an administrator or the user. It wins over the
    /// browser's zone under `TimePolicy::User`.
    pub time_zone: Option<String>,
    /// When the user last proved who they are (signed in), as reported by the identity provider.
    /// Actions marked `reauth` need this to be recent.
    pub authenticated_at: Option<crate::time::DateTime>,
    /// The user proved a second factor (a one time code, a security key) in this session, as the
    /// identity provider reports it (`amr`, `acr`) or as the built in TOTP check records it.
    /// `RequireMfa` guards need it.
    pub mfa: bool,
}

impl Identity {
    pub fn anonymous() -> Self {
        Self::default()
    }

    pub fn user(name: impl Into<String>) -> Self {
        Self { user: Some(name.into()), ..Self::default() }
    }

    pub fn with_permissions<I, S>(mut self, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.permissions.extend(permissions.into_iter().map(Into::into));
        self
    }

    /// Identity that may do everything. Only for development and tests.
    pub fn superuser(name: impl Into<String>) -> Self {
        Self { user: Some(name.into()), superuser: true, ..Self::default() }
    }

    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = Some(tenant.into());
        self
    }

    pub fn with_time_zone(mut self, zone: impl Into<String>) -> Self {
        self.time_zone = Some(zone.into());
        self
    }

    pub fn with_authenticated_at(mut self, at: crate::time::DateTime) -> Self {
        self.authenticated_at = Some(at);
        self
    }

    pub fn is_signed_in(&self) -> bool {
        self.user.is_some()
    }

    /// Whether this identity passes the access rule.
    pub fn can(&self, access: &Access) -> bool {
        match access {
            Access::Public => true,
            Access::Permission(p) => self.superuser || self.permissions.contains(*p),
        }
    }
}

/// Marks a type as an injectable service. Register it with `App::service`.
///
/// ```ignore
/// #[derive(Clone)]
/// pub struct ProductStore { ... }
/// impl Service for ProductStore {}
/// ```
pub trait Service: Clone + Send + Sync + 'static {}

/// Registered services, looked up by type.
#[derive(Default)]
pub struct Services {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    names: HashMap<TypeId, &'static str>,
}

impl Services {
    pub fn insert<T: Service>(&mut self, service: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(service));
        self.names.insert(TypeId::of::<T>(), std::any::type_name::<T>());
    }

    pub fn get<T: Service>(&self) -> Option<T> {
        self.map.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref::<T>()).cloned()
    }

    pub fn contains(&self, id: TypeId) -> bool {
        self.map.contains_key(&id)
    }
}

/// A dependency of an action or screen, checked when the app starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dep {
    pub id: TypeId,
    pub name: &'static str,
}

impl Dep {
    pub fn of<T: 'static>() -> Self {
        Dep { id: TypeId::of::<T>(), name: std::any::type_name::<T>() }
    }
}

struct CtxInner {
    identity: Identity,
    locale: String,
    tz: TimeZone,
    now: Option<DateTime>,
    tenant: Option<Tenant>,
    license: Option<Arc<License>>,
    config: Config,
    services: Arc<Services>,
    i18n: Arc<I18n>,
    via_agent: bool,
    trail: Arc<Mutex<crate::audit::Trail>>,
}

/// Request context: identity, language and services. Cheap to clone. Actions and screens may take
/// it as an injected argument.
#[derive(Clone)]
pub struct Ctx(Arc<CtxInner>);

impl Ctx {
    pub fn new(identity: Identity, locale: impl Into<String>, services: Arc<Services>, i18n: Arc<I18n>) -> Self {
        Ctx(Arc::new(CtxInner {
            identity,
            locale: locale.into(),
            tz: TimeZone::UTC,
            now: None,
            tenant: None,
            license: None,
            config: Config::default(),
            services,
            i18n,
            via_agent: false,
            trail: Arc::default(),
        }))
    }

    /// Same context in another time zone.
    pub fn with_time_zone(&self, tz: TimeZone) -> Self {
        self.rebuild(|i| i.tz = tz)
    }

    /// Same context with a fixed clock. For tests.
    pub fn with_now(&self, now: DateTime) -> Self {
        self.rebuild(|i| i.now = Some(now))
    }

    fn rebuild(&self, change: impl FnOnce(&mut CtxInner)) -> Self {
        let i = &self.0;
        let mut inner = CtxInner {
            identity: i.identity.clone(),
            locale: i.locale.clone(),
            tz: i.tz,
            now: i.now,
            tenant: i.tenant.clone(),
            license: i.license.clone(),
            config: i.config.clone(),
            services: i.services.clone(),
            i18n: i.i18n.clone(),
            via_agent: i.via_agent,
            trail: i.trail.clone(),
        };
        change(&mut inner);
        Ctx(Arc::new(inner))
    }

    pub fn with_tenant(&self, tenant: Option<Tenant>) -> Self {
        self.rebuild(|i| i.tenant = tenant)
    }

    pub fn with_license(&self, license: Option<Arc<License>>) -> Self {
        self.rebuild(|i| i.license = license)
    }

    pub fn with_config(&self, config: Config) -> Self {
        self.rebuild(|i| i.config = config)
    }

    /// The tenant of the request in multi tenant apps.
    pub fn tenant(&self) -> Option<&Tenant> {
        self.0.tenant.as_ref()
    }

    /// The license that applies to the request, if the app uses licenses.
    pub fn license(&self) -> Option<&License> {
        self.0.license.as_deref()
    }

    /// App configuration loaded at startup.
    pub fn config(&self) -> &Config {
        &self.0.config
    }

    /// The user's time zone.
    pub fn time_zone(&self) -> TimeZone {
        self.0.tz
    }

    /// The current moment.
    pub fn now(&self) -> DateTime {
        self.0.now.unwrap_or_else(DateTime::now)
    }

    /// Today's date in the user's time zone.
    pub fn today(&self) -> Date {
        self.time_zone().today(self.now())
    }

    /// Context for tests: the given identity, English, built in catalogs and the given services.
    pub fn for_tests(identity: Identity, services: Services) -> Self {
        Ctx::new(identity, "en", Arc::new(services), Arc::new(I18n::new("en")))
    }

    /// Same context, marked as a call made by an AI agent (WebMCP or MCP). Audit logs record it.
    pub fn as_agent(&self) -> Self {
        self.rebuild(|i| i.via_agent = true)
    }

    pub fn identity(&self) -> &Identity {
        &self.0.identity
    }

    pub fn locale(&self) -> &str {
        &self.0.locale
    }

    pub fn i18n(&self) -> &I18n {
        &self.0.i18n
    }

    pub fn via_agent(&self) -> bool {
        self.0.via_agent
    }

    pub fn service<T: Service>(&self) -> Option<T> {
        self.0.services.get::<T>()
    }

    /// What this call did to data so far: the saved record and its changed fields.
    pub fn trail(&self) -> crate::audit::Trail {
        self.0.trail.lock().map(|t| t.clone()).unwrap_or_default()
    }

    pub(crate) fn update_trail(&self, change: impl FnOnce(&mut crate::audit::Trail)) {
        if let Ok(mut t) = self.0.trail.lock() {
            change(&mut t);
        }
    }

    /// Records the fields that differ between the record as loaded from the store and as saved,
    /// for the audit log. Call it in save actions with the record read from the database; forms do
    /// it on their own with the record their screen loaded.
    ///
    /// ```ignore
    /// let current = store.get(p.id)?;
    /// ctx.record_changes(&current, &p);
    /// ```
    pub fn record_changes<M: crate::model::Model>(&self, loaded: &M, saved: &M) {
        let changes = crate::audit::diff(loaded, saved);
        let record = crate::audit::RecordRef::of(saved);
        self.update_trail(|t| {
            t.record = Some(record);
            t.changes = changes;
        });
    }

    /// Records that the user was shown a sensitive record, for the audit log. Forms, grids and API
    /// output of models marked `sensitive` do this on their own.
    pub fn record_read<M: crate::model::Model>(&self, record: &M) {
        self.note_read(crate::audit::RecordRef::of(record));
    }

    pub(crate) fn note_read(&self, record: crate::audit::RecordRef) {
        self.update_trail(|t| {
            if !t.reads.contains(&record) {
                t.reads.push(record);
            }
        });
    }

    /// Resolves a text in the request's language.
    pub fn t(&self, text: &Text) -> String {
        self.0.i18n.text(&self.0.locale, text)
    }
}

/// Types that can be injected into actions and screens: `Ctx` and registered services.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be injected",
    label = "arguments after the first are injected",
    note = "injected arguments must be `Ctx` or a type that implements `Service` (register it with `App::service`)"
)]
pub trait Inject: Sized + Send + 'static {
    fn inject(ctx: &Ctx) -> Result<Self>;
    fn dep() -> Option<Dep>;
}

impl Inject for Ctx {
    fn inject(ctx: &Ctx) -> Result<Self> {
        Ok(ctx.clone())
    }

    fn dep() -> Option<Dep> {
        None
    }
}

impl<T: Service> Inject for T {
    fn inject(ctx: &Ctx) -> Result<Self> {
        ctx.service::<T>()
            .ok_or_else(|| Error::Internal(format!("service `{}` is not registered", std::any::type_name::<T>())))
    }

    fn dep() -> Option<Dep> {
        Some(Dep::of::<T>())
    }
}

/// What a screen function may return.
pub trait IntoScreen {
    fn into_screen(self) -> Result<Screen>;
}

impl IntoScreen for Screen {
    fn into_screen(self) -> Result<Screen> {
        Ok(self)
    }
}

impl IntoScreen for Result<Screen> {
    fn into_screen(self) -> Result<Screen> {
        self
    }
}

/// Action outputs that can report failure to a form or grid.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a supported action result",
    label = "this action does not return a `Result`",
    note = "actions used by forms, grid row actions, menus and bulk actions return `zeyn::Result<T>`"
)]
pub trait Outcome {
    fn failure(&self) -> Option<Error>;
}

impl<T> Outcome for Result<T> {
    fn failure(&self) -> Option<Error> {
        self.as_ref().err().cloned()
    }
}
