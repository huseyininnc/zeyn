//! Zeyn component catalog site.
//!
//! Run: `cargo run -p zeyn-example-components`, then open http://127.0.0.1:3010. The address
//! comes from `ZEYN_COMPONENTS_SERVER__ADDR` when set.

use zeyn::{Config, EnvSource, StaticSource};
use zeyn_example_components::{app, site};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let defaults = StaticSource(
        [("server.addr".to_string(), "127.0.0.1:3010".to_string())]
            .into_iter()
            .collect(),
    );
    let config = Config::load(vec![
        Box::new(defaults),
        Box::new(EnvSource("ZEYN_COMPONENTS")),
    ])
    .await
    .map_err(std::io::Error::other)?;
    let addr = config
        .get("server.addr")
        .unwrap_or("127.0.0.1:3010")
        .to_string();
    let router = app()
        .map_err(std::io::Error::other)?
        .config(config)
        .router()
        .map_err(|problems| std::io::Error::other(problems.join("; ")))?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Zeyn components on http://{}", listener.local_addr()?);
    axum::serve(
        listener,
        site::router(router).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
}
