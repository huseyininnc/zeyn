//! An app that uses the component: the start page holds none and does not link its browser
//! module; the second page holds one, so the module loads there, also after client navigation
//! from the start page.
//!
//! Run: `cargo run --example demo`, then open http://127.0.0.1:3011

use zeyn::I18n;
use zeyn::prelude::*;
use {{crate}}::{{{type}}, {{type}}Plugin};

#[screen(route = "/", permission = "demo.read")]
pub fn start() -> Screen {
    Screen::new(t!("demo.home")).body(Paragraph::new(t!("demo.intro")))
}

#[screen(route = "/component", permission = "demo.read")]
pub fn component() -> Screen {
    Screen::new(t!("demo.page")).body({{type}}::new(t!("demo.label")))
}

/// The demo app.
pub fn app() -> App {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../../locales/tr.json")))
        .expect("catalogs are valid JSON");
    App::new(t!("demo.title"))
        .i18n(i18n)
        .plugin({{type}}Plugin)
        .home(start)
        .feature(
            Feature::new("demo")
                .screen(start)
                .screen(component)
                .menu(|m| {
                    m.group(t!("demo.menu"), Icon::Table)
                        .item(t!("demo.home"), start)
                        .item(t!("demo.page"), component)
                }),
        )
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let addr = std::env::var("ZEYN_ADDR").unwrap_or_else(|_| "127.0.0.1:3011".into());
    app()
        .auth(DevAuth::superuser("demo"))
        .allow_dev_auth()
        .serve(&addr)
        .await
}
