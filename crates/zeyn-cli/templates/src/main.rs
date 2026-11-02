//! Starts the app: configuration first, then the server.

mod app;
mod features;

use zeyn::{Config, EnvSource, JsonFileSource};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::load(vec![
        Box::new(JsonFileSource("config/app.json".into())),
        Box::new(EnvSource("APP")),
    ])
    .await
    .map_err(std::io::Error::other)?;
    let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
    app::app(config)?.serve(&addr).await
}
