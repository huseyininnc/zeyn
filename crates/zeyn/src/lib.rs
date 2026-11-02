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
//! pub async fn search_products(_q: GridQuery) -> Result<Page<Product>> {
//!     Ok(Page::default())
//! }
//!
//! #[screen(route = "/products", permission = "stock.product.read")]
//! pub fn product_list() -> Screen {
//!     Screen::new("Products").body(Grid::of(search_products).column(Product::code).filter())
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
//! pub async fn search_products(_q: GridQuery) -> Result<Page<Product>> { Ok(Page::default()) }
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
//! pub async fn search_products(_q: GridQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { Grid::of(search_products).column(Product::code).sum().into() }
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
//! ## Extension points
//!
//! - [`Component`]: any type that renders HTML and describes itself in the schema can go on a screen.
//! - [`FieldType`]: new field types; use [`FieldKind::Custom`] to name a client side editor.
//! - Models, actions and screens from other crates are used like local ones.

mod action;
mod api;
mod ctx;
mod feature;
mod form;
mod grid;
mod i18n;
mod model;
mod platform;
pub mod render;
mod screen;
#[cfg(feature = "server")]
mod security;
mod time;
#[cfg(feature = "server")]
pub mod server;
mod value;

pub use action::{
    Access, Action, ActionMeta, Cursor, DEFAULT_PAGE_SIZE, Error, Filter, GridQuery, MAX_PAGE_SIZE, Page, PageRequest,
    Result, Sort,
};
pub use api::{ApiEntry, ApiInput, ApiOutput, ApiResult, model_schema};
pub use ctx::{BoxFuture, Ctx, Dep, Identity, Inject, IntoScreen, Outcome, Service, Services};
pub use feature::{Feature, Guard, GuardResult, Menu, MenuGroup, MenuItem, Route, ScreenEntry, SignedIn};
pub use form::{Form, FormAction, FormRow, SubmitForm};
pub use grid::{Grid, GridColumn, GridMode, Paging, RowStyle, position_cursor};
pub use i18n::{I18n, NumberFormat, Text};
pub use model::{__rules, Entity, Field, FieldError, FieldMeta, Model, Rule, Valid, ValidationErrors};
pub use platform::{
    Config, ConfigSource, EnvSource, FixedLicense, JsonFileSource, License, LicenseSource, Licensed, PerTenant, Secret,
    StaticSource, Tenant, TenantRequired,
};
pub use render::Render;
pub use time::{Date, DateTime, LocalDateTime, LocalTimeError, Time, TimeZone};
#[cfg(feature = "server")]
pub use server::{
    Agents, ApiDocs, App, AuditLog, Authenticator, BasicCredentials, DevAuth, ForwardAuth, HeaderTenant, Interceptor, MemoryAudit, Plugin, StaticCredentials, StderrAudit,
    SubdomainTenant, TenantResolver, Theme, TimePolicy,
};
#[cfg(feature = "server")]
pub use security::{IpRange, MemoryRateStore, RateKey, RateLimit, RateStore, client_ip, parse_ranges};
pub use screen::{
    Component, Effect, Export, FormSource, GridSource, Icon, LoadedPage, Node, Params, Screen, ScreenDef, ScreenMeta,
    ToolButton, Toolbar, UiCtx, Window,
};
pub use value::{
    ChoiceOption, Currency, FieldKind, FieldType, Money, SupportsMinMax, SupportsRequired, SupportsSum, SupportsText,
    Value,
};
pub use zeyn_macros::{action, choice, entity, model, params, screen, t};

#[doc(hidden)]
pub use serde as __serde;
#[doc(hidden)]
pub use serde_json as __serde_json;

/// Everything application code needs: `use zeyn::prelude::*;`
pub mod prelude {
    pub use crate::{
        Action, Ctx, Currency, Date, DateTime, Effect, LocalDateTime, Time, TimeZone, Entity, Error, Export, Feature, Field, Form, Grid, GridMode, GridQuery,
        GuardResult, Icon, Identity, Licensed, Model, Secret, Tenant, TenantRequired, Money, Node, Page, Paging, Result, RowStyle, Screen, ScreenDef, Service,
        SignedIn, Text, UiCtx, Valid, ValidationErrors, Window, action, choice, entity, model, params, screen, t,
    };
    #[cfg(feature = "server")]
    pub use crate::{
        Agents, ApiDocs, App, AuditLog, DevAuth, MemoryAudit, Plugin, RateLimit, StaticCredentials, StderrAudit, Theme,
        TimePolicy,
    };
}
