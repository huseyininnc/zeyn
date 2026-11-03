//! The application: languages, services, security and features.

use crate::features;
use std::time::Duration;
use zeyn::prelude::*;
use zeyn::{Config, I18n};

/// Builds the app from its configuration. Debug builds sign every request in as a developer with
/// full access (`DevAuth`). Release builds have no sign in until a real authenticator is added
/// (`OidcAuth`, `JwtAuth`, `ForwardAuth` or `App::accounts`), so every screen that needs a
/// permission is refused and the API docs need the `api.docs` permission.
pub fn app(config: Config) -> std::io::Result<App> {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .map_err(std::io::Error::other)?;
    let zone = config
        .get("time.zone")
        .and_then(TimeZone::named)
        .unwrap_or(TimeZone::UTC);
    let mut app = App::new(t!("app.name"))
        .config(config.clone())
        .i18n(i18n)
        .time_policy(TimePolicy::User { default: zone })
        .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
        .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
        .interceptor(AuditLog(StderrAudit))
        .mcp(Mcp::read_only())
        .service(features::notes::store::NoteStore::default())
        .home(features::home::home)
        .feature(features::home::feature())
        .feature(features::notes::feature());
    if cfg!(debug_assertions) {
        app = app.auth(DevAuth::superuser("developer"));
    }
    if config.get("docs.mode") == Some("open") {
        app = app.api_docs(if cfg!(debug_assertions) {
            ApiDocs::open()
        } else {
            ApiDocs::permission("api.docs")
        });
    }
    Ok(app)
}
