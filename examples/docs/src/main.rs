//! Zeyn documentation site.
//!
//! Run: `cargo run -p zeyn-example-docs`, then open http://127.0.0.1:3008. The address comes from
//! `ZEYN_DOCS_SERVER__ADDR` when set.

use zeyn::{Config, EnvSource, StaticSource};
use zeyn_example_docs::{Docs, app};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let defaults = StaticSource([("server.addr".to_string(), "127.0.0.1:3008".to_string())].into_iter().collect());
    let config = Config::load(vec![Box::new(defaults), Box::new(EnvSource("ZEYN_DOCS"))]).await.map_err(std::io::Error::other)?;
    let addr = config.get("server.addr").unwrap_or("127.0.0.1:3008").to_string();
    app(Docs::build()).map_err(std::io::Error::other)?.config(config).serve(&addr).await
}
