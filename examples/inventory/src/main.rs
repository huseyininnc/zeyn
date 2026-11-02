//! Inventory: product list with server paging, filters and an editor window with row version
//! checks, in English and Turkish.
//!
//! Run: `cargo run -p zeyn-example-inventory`, then open http://127.0.0.1:3001
//!
//! Products are kept in memory by default. Set `ZEYN_DATABASE_URL` to a PostgreSQL
//! (`postgres://...`) or Oracle (`oracle://...`) URL to keep them in that database instead; the
//! table is created and filled on the first start.

mod stock;

use stock::store::ProductStore;
use zeyn::I18n;
use zeyn::prelude::*;

/// The application: catalogs, services, features.
fn app(store: ProductStore) -> App {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .expect("catalogs are valid JSON");
    App::new(t!("app.name"))
        .i18n(i18n)
        .service(store)
        .auth(DevAuth::superuser("demo"))
        .interceptor(AuditLog(StderrAudit))
        .agents(Agents::read_write())
        .home(stock::screens::product_list)
        .feature(stock::feature())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("ZEYN_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".into());
    let store = match std::env::var("ZEYN_DATABASE_URL") {
        Ok(url) if !url.is_empty() => ProductStore::connect(&url).await.map_err(std::io::Error::other)?,
        _ => ProductStore::seeded(),
    };
    app(store).serve(&addr).await
}
