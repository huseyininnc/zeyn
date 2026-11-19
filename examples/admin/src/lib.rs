//! Admin: a back office template on a database. A dashboard with key figures and charts, orders
//! with filters, saved views, export and a history, customers, a product catalog, company
//! settings, users and roles, live updates between open pages and a command palette.
//!
//! Run: `cargo run -p zeyn-example-admin`, then open http://127.0.0.1:3011
//!
//! The records are kept in memory with sample data unless `ZEYN_ADMIN_DATABASE__URL` names a
//! PostgreSQL database and the app is built with `--features postgres`; see the README.

pub mod features;
pub mod models;
pub mod roles;
pub mod store;

use std::time::Duration;
use store::Store;
use zeyn::prelude::*;
use zeyn::realtime::ZeynSocket;
use zeyn::{Authenticator, Config, I18n, MemoryViewStore};

/// The topic named whenever an order changes: dashboards, order grids and the menu count refresh
/// in place on every open page of users who may read orders.
pub const ORDERS_TOPIC: &str = "orders";

/// Builds the app from its configuration, its store and how users sign in.
pub fn app(config: Config, store: Store, auth: impl Authenticator) -> std::io::Result<App> {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .map_err(std::io::Error::other)?;
    let zone = config
        .get("time.zone")
        .and_then(TimeZone::named)
        .unwrap_or(TimeZone::UTC);
    let live = ZeynSocket::new().topic(ORDERS_TOPIC, "admin.orders.read");
    Ok(App::new(t!("app.name"))
        .config(config)
        .i18n(i18n)
        .time_policy(TimePolicy::User { default: zone })
        .time_zone_picker()
        .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
        .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
        .interceptor(AuditLog(StderrAudit))
        .auth(auth)
        .header(|h| h.breadcrumb().search().account())
        .service(store)
        .socket(live)
        .datagrid_views(MemoryViewStore::default())
        .home(features::dashboard::dashboard)
        .feature(features::dashboard::feature())
        .feature(features::orders::feature())
        .feature(features::customers::feature())
        .feature(features::products::feature())
        .feature(features::admin::feature()))
}

/// Whether the signed in user holds a permission, for parts of a screen that only some roles
/// see. The server checks the permission again on every action.
pub fn can(ctx: &Ctx, permission: &str) -> bool {
    let identity = ctx.identity();
    identity.superuser
        || identity
            .permissions
            .contains(permission)
}
