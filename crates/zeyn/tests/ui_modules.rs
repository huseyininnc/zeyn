//! Browser modules: a page links the scripts and styles of the components it holds and nothing
//! else; the runtime gets the same table for content that arrives later.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity};

#[entity(table = "trips")]
pub struct Trip {
    #[key]
    pub id: TripId,
    pub name: String,
    pub day: Option<Date>,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Trip>>>);

impl Service for Store {}

#[action(permission = "trips.read")]
pub async fn search_trips(q: DataQuery, store: Store) -> Result<Page<Trip>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[screen(route = "/plain", permission = "trips.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/trips", permission = "trips.read")]
pub fn trips() -> Screen {
    Screen::new("Trips").body(
        DataGrid::of(search_trips)
            .column(Trip::name)
            .column(Trip::day),
    )
}

#[screen(route = "/trip", permission = "trips.read")]
pub fn trip() -> Screen {
    Screen::new("Trip").body(
        Form::of(Trip::default())
            .field(Trip::name)
            .field(Trip::day),
    )
}

#[screen(route = "/photo", permission = "trips.read")]
pub fn photo() -> Screen {
    Screen::new("Photo").body(
        Image::new("/p.webp")
            .alt("Photo")
            .size(40, 40),
    )
}

#[screen(route = "/photos", permission = "trips.read")]
pub fn photos() -> Screen {
    Screen::new("Photos").body(
        Image::new("/p.webp")
            .alt("Photo")
            .size(40, 40)
            .placeholder(Placeholder::color("#d9dde3")),
    )
}

fn app() -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(Store::default())
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["trips.read"])))
        .feature(
            Feature::new("trips")
                .screen(plain)
                .screen(trips)
                .screen(trip)
                .screen(photo)
                .screen(photos),
        )
        .router()
        .unwrap()
}

async fn get(path: &str) -> (StatusCode, String) {
    let res = app()
        .oneshot(
            Request::get(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// The module table `zeyn.js` is served with: `(name, script selector, style selector)`.
async fn table() -> Vec<(String, String, String)> {
    let (status, js) = get("/_zeyn/zeyn.js").await;
    assert_eq!(status, StatusCode::OK);
    let start = js
        .find("const MODULES = ")
        .expect("the runtime carries the module table")
        + "const MODULES = ".len();
    let end = start + js[start..].find(";\n").unwrap();
    serde_json::from_str(&js[start..end]).expect("the table is JSON")
}

/// Names of the framework scripts and styles a page links, such as `grid.js` or `charts.css`.
fn links(page: &str) -> Vec<String> {
    page.split("/_zeyn/")
        .skip(1)
        .filter_map(|s| {
            let name = s.split('?').next()?;
            (name.ends_with(".js") || name.ends_with(".css")).then(|| name.to_string())
        })
        .collect()
}

#[tokio::test]
async fn a_screen_without_a_component_never_references_its_module() {
    let (status, page) = get("/plain").await;
    assert_eq!(status, StatusCode::OK);
    let linked = links(&page);
    let core = [
        "zeyn.css",
        "zeyn.js",
        "nav.js",
        "enterprise.css",
        "enterprise.js",
        "components.css",
        "content.css",
        "pro.css",
        "components.js",
        "palette.js",
    ];
    for name in &linked {
        assert!(
            core.contains(&name.as_str()),
            "a page with only text links {name}"
        );
    }
    for (name, script, style) in table().await {
        let on_shell = name == "components" || name == "palette";
        if !script.is_empty() && !on_shell {
            assert!(
                !linked.contains(&format!("{name}.js")),
                "{name}.js is linked by a page without {script}"
            );
        }
        if !style.is_empty() {
            assert!(
                !linked.contains(&format!("{name}.css")),
                "{name}.css is linked by a page without {style}"
            );
        }
    }
    assert!(!linked.contains(&"grid.js".to_string()));
}

#[tokio::test]
async fn a_page_links_the_modules_of_its_components_with_what_they_import() {
    let (_, grid) = get("/trips").await;
    assert!(grid.contains(r#"<link rel="modulepreload" href="/_zeyn/grid.js?v="#));
    assert!(!links(&grid).contains(&"dates.js".to_string()));
    let (_, form) = get("/trip").await;
    let linked = links(&form);
    for name in ["dates.js", "components.js"] {
        assert!(linked.contains(&name.to_string()), "{name}");
    }
    assert!(!linked.contains(&"grid.js".to_string()));
    assert!(!linked.contains(&"lines.js".to_string()));
    let at = |name: &str| {
        form.find(&format!("/_zeyn/{name}?"))
            .unwrap()
    };
    assert!(
        at("components.js") < at("dates.js"),
        "a module comes after what it imports"
    );
}

#[tokio::test]
async fn every_module_in_the_table_is_served_and_imports_the_shared_part_by_its_own_build() {
    for (name, script, style) in table().await {
        if !script.is_empty() {
            let (status, body) = get(&format!("/_zeyn/{name}.js")).await;
            assert_eq!(status, StatusCode::OK, "{name}.js");
            assert!(body.contains("@module"), "{name}.js");
            assert!(
                !body.contains("_zeyn/components.js'") && !body.contains("from './"),
                "{name}.js imports other modules with the build query of its own URL"
            );
        }
        if !style.is_empty() {
            let (status, _) = get(&format!("/_zeyn/{name}.css")).await;
            assert_eq!(status, StatusCode::OK, "{name}.css");
        }
    }
}

#[tokio::test]
async fn every_rule_of_a_module_stylesheet_is_under_one_of_its_markers() {
    for (name, _, style) in table().await {
        if style.is_empty() {
            continue;
        }
        let markers: Vec<&str> = style
            .split(", ")
            .map(|s| s.trim_start_matches('.'))
            .collect();
        let (_, raw) = get(&format!("/_zeyn/{name}.css")).await;
        let mut css = String::with_capacity(raw.len());
        let mut text = raw.as_str();
        while let Some(start) = text.find("/*") {
            css.push_str(&text[..start]);
            text = text[start..]
                .find("*/")
                .map_or("", |end| &text[start + end + 2..]);
        }
        css.push_str(text);
        let mut rest = css.as_str();
        while let Some(open) = rest.find('{') {
            let selectors = rest[..open]
                .rsplit(['}', ';'])
                .next()
                .unwrap()
                .trim();
            rest = &rest[open + 1..];
            if selectors.starts_with('@') || selectors.is_empty() || !selectors.contains('.') {
                continue;
            }
            for selector in selectors.split(',') {
                let covered = selector
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                    .any(|token| {
                        markers
                            .iter()
                            .any(|m| token.starts_with(m))
                    });
                assert!(
                    covered,
                    "{name}.css styles `{}` without one of the markers {style}; add the component's class to `style` of its module in server/modules.rs",
                    selector.trim()
                );
            }
        }
    }
}

#[tokio::test]
async fn the_media_script_comes_only_with_images_that_need_it() {
    let (_, simple) = get("/photo").await;
    let linked = links(&simple);
    assert!(linked.contains(&"media.css".to_string()));
    assert!(
        !linked.contains(&"media.js".to_string()),
        "a plain image needs no script"
    );
    let (_, progressive) = get("/photos").await;
    let linked = links(&progressive);
    assert!(linked.contains(&"media.css".to_string()) && linked.contains(&"media.js".to_string()));
    let table = table().await;
    let media = table
        .iter()
        .find(|(name, _, _)| name == "media")
        .unwrap();
    assert!(media.1.contains("[data-zeyn-img]") && media.2.contains(".z-img-host"));
}
