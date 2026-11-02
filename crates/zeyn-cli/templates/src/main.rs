//! Starts the app: configuration first, then the server. `<app> health` asks a running app whether
//! it is alive, for container health checks.

mod app;
mod features;

use zeyn::{Config, EnvSource, JsonFileSource};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))])
        .await
        .map_err(std::io::Error::other)?;
    let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
    if std::env::args().nth(1).as_deref() == Some("health") {
        return zeyn::probe(&addr);
    }
    app::app(config)?.serve(&addr).await
}
