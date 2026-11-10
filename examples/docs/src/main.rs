//! Zeyn documentation site.
//!
//! Run: `cargo run -p zeyn-example-docs`, then open http://127.0.0.1:3008. Settings come from
//! `ZEYN_DOCS_*` variables: `ZEYN_DOCS_SERVER__ADDR` for the address, and `ZEYN_DOCS_VERSION`,
//! `ZEYN_DOCS_VERSIONS`, `ZEYN_DOCS_LATEST` and `ZEYN_DOCS_BASE_PATH` for versioned docs (see
//! `docs/operations.md`). The app is served through [`zeyn_example_docs::router`], which adds
//! the version switch, so this binary installs the log subscriber itself.

use zeyn::{Config, EnvSource, StaticSource};
use zeyn_example_docs::versions::Versions;
use zeyn_example_docs::{Docs, app, router};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let defaults = StaticSource(
        [("server.addr".to_string(), "127.0.0.1:3008".to_string())]
            .into_iter()
            .collect(),
    );
    let config = Config::load(vec![Box::new(defaults), Box::new(EnvSource("ZEYN_DOCS"))])
        .await
        .map_err(std::io::Error::other)?;
    let addr = config
        .get("server.addr")
        .unwrap_or("127.0.0.1:3008")
        .to_string();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let versions = Versions::from_config(&config);
    let app = app(Docs::build(versions.base()), versions.clone())
        .map_err(std::io::Error::other)?
        .config(config);
    let router = router(app, &versions).map_err(|p| std::io::Error::other(p.join("\n")))?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(
        "Zeyn docs {} listen on http://{}{}",
        versions.current(),
        listener.local_addr()?,
        versions.base()
    );
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
}
