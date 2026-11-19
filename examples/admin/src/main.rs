//! Starts the admin app: configuration, the store, then the server.

use zeyn::prelude::*;
use zeyn::{Config, EnvSource, JsonFileSource};
use zeyn_example_admin::roles::Role;
use zeyn_example_admin::store::Store;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::load(vec![
        Box::new(JsonFileSource(
            concat!(env!("CARGO_MANIFEST_DIR"), "/config/app.json").into(),
        )),
        Box::new(EnvSource("ZEYN_ADMIN")),
    ])
    .await
    .map_err(std::io::Error::other)?;
    let addr = config
        .get("server.addr")
        .unwrap_or("127.0.0.1:3011")
        .to_string();
    let role = config
        .get("auth.role")
        .and_then(Role::named)
        .unwrap_or(Role::Admin);
    let store = store(&config).await?;
    let auth = DevAuth::identity(role.identity("Demo user"));
    zeyn_example_admin::app(config, store, auth)?
        .allow_dev_auth()
        .serve(&addr)
        .await
}

/// PostgreSQL when `database.url` is set, else the sample data in memory.
async fn store(config: &Config) -> std::io::Result<Store> {
    match config.get("database.url") {
        #[cfg(feature = "postgres")]
        Some(url) if !url.is_empty() => Store::connect(config)
            .await
            .map_err(|e| std::io::Error::other(format!("{e:?}"))),
        #[cfg(not(feature = "postgres"))]
        Some(url) if !url.is_empty() => Err(std::io::Error::other(
            "ZEYN_ADMIN_DATABASE__URL is set; build with --features postgres",
        )),
        _ => Ok(Store::sample(TimeZone::UTC.today(DateTime::now()))),
    }
}
