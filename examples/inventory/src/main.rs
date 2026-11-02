//! Inventory: product list with server paging, filters and an editor window with row version
//! checks, in English and Turkish.
//!
//! Run: `cargo run -p zeyn-example-inventory`, then open http://127.0.0.1:3001

mod stock;

use zeyn::I18n;
use zeyn::prelude::*;

/// The application: catalogs, services, features.
fn app() -> App {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .expect("catalogs are valid JSON");
    App::new(t!("app.name"))
        .i18n(i18n)
        .service(stock::store::ProductStore::seeded())
        .auth(DevAuth::superuser("demo"))
        .interceptor(AuditLog(StderrAudit))
        .agents(Agents::read_write())
        .home(stock::screens::product_list)
        .feature(stock::feature())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("ZEYN_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".into());
    app().serve(&addr).await
}
