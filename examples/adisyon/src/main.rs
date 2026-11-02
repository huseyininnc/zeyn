//! Starts the app: configuration first, then the server, or with the `desktop` feature a
//! native window around it.

#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod features;
mod restaurant;

#[cfg(not(feature = "desktop"))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    use zeyn::{Config, EnvSource, JsonFileSource};
    let config = Config::load(vec![
        Box::new(JsonFileSource(concat!(env!("CARGO_MANIFEST_DIR"), "/config/app.json").into())),
        Box::new(EnvSource("APP")),
    ])
    .await
    .map_err(std::io::Error::other)?;
    let addr = config.get("server.addr").unwrap_or("127.0.0.1:3006").to_string();
    app::app(config)?.serve(&addr).await
}

/// The desktop exe carries its configuration inside, so it runs from any folder and offline.
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    use zeyn::{Config, EnvSource, StaticSource};
    let defaults = StaticSource([("time.zone".to_string(), "Europe/Istanbul".to_string())].into());
    let load = Config::load(vec![Box::new(defaults), Box::new(EnvSource("APP"))]);
    let config =
        tokio::runtime::Builder::new_current_thread().build()?.block_on(load).map_err(std::io::Error::other)?;
    zeyn_desktop::Desktop::new(app::app(config)?).id("zeyn-adisyon").title("Adisyon").size(1280, 800).run()
}
