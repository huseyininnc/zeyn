//! {{title}}: shared models, services and features used by several Zeyn apps.
//!
//! An app uses it in three lines:
//!
//! ```ignore
//! let i18n = I18n::new("en").catalogs({{crate}}::CATALOGS)?.catalogs(APP_CATALOGS)?;
//! App::new("Billing").i18n(i18n)
//!     .service({{crate}}::customers::CustomerStore::default())
//!     .feature({{crate}}::customers::feature())
//! ```
//!
//! Every app must use the same Zeyn version as this library, or the shared types will not match.

pub mod customers;

/// Texts of this library in every language it ships. Load them before the app's own catalogs.
pub const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en.json")),
    ("tr", include_str!("../locales/tr.json")),
];
