//! Size budgets of the browser code. Every module file stays under a gzip cap, and each kind of
//! page stays under a budget for everything it links (scripts, stylesheets and what the runtime
//! preloads). A change that crosses one either earns its bytes, with the before and after sizes
//! in the commit, and raises the budget here, or gets smaller.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::io::Write;
use tower::ServiceExt;
use zeyn::prelude::*;

/// Largest module file, gzip compressed, apart from the core and the grid runtime.
const MODULE_GZIP: usize = 12 * 1024;

/// What a page links in all, gzip compressed, by kind of page. Measured on 2026-11-12 at 81 KB
/// (plain screen), 109 KB (grid) and 83 KB (form), with about 8 percent of room. The grid page
/// grew to 118.3 KB by 2026-11-13 (page sizes, autofit, filter groups) and has 120 KB.
const PLAIN_PAGE_GZIP: usize = 88 * 1024;
const GRID_PAGE_GZIP: usize = 120 * 1024;
const FORM_PAGE_GZIP: usize = 92 * 1024;

#[model]
pub struct Note {
    #[rule(required, max_len = 40)]
    pub title: String,
}

#[entity(table = "rows")]
pub struct Row {
    #[key]
    pub id: RowId,
    pub name: String,
}

#[action(permission = "x.read")]
pub async fn rows(_q: DataQuery) -> zeyn::Result<Page<Row>> {
    Ok(Page::new(Vec::new()))
}

#[action(permission = "x.read")]
pub async fn save_note(n: Valid<Note>) -> zeyn::Result<()> {
    let _ = n.into_inner();
    Ok(())
}

#[screen(route = "/plain", permission = "x.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Text"))
}

#[screen(route = "/grid", permission = "x.read")]
pub fn grid() -> Screen {
    Screen::new("Grid").body(DataGrid::of(rows).column(Row::name))
}

#[screen(route = "/form", permission = "x.read")]
pub fn form() -> Screen {
    Screen::new("Form").body(
        Form::of(Note::default())
            .field(Note::title)
            .submit("Save", save_note),
    )
}

fn app() -> axum::Router {
    App::new("Budget")
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["x.read"])))
        .feature(
            Feature::new("x")
                .screen(plain)
                .screen(grid)
                .screen(form),
        )
        .router()
        .unwrap()
}

async fn get(router: &axum::Router, path: &str) -> (StatusCode, Vec<u8>) {
    let res = router
        .clone()
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
        .to_bytes()
        .to_vec();
    (status, bytes)
}

fn gzip(bytes: &[u8]) -> usize {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(bytes).unwrap();
    e.finish().unwrap().len()
}

/// The framework scripts and stylesheets a page links or preloads, as served paths.
fn linked(page: &str) -> Vec<String> {
    let mut paths: Vec<String> = page
        .split("/_zeyn/")
        .skip(1)
        .filter_map(|s| {
            let name = s.split(['?', '"']).next()?;
            (name.ends_with(".js") || name.ends_with(".css")).then(|| format!("/_zeyn/{name}"))
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The gzip size of everything a page links, with the list for the failure message.
async fn page_weight(router: &axum::Router, path: &str) -> (usize, Vec<(String, usize)>) {
    let (status, page) = get(router, path).await;
    assert_eq!(status, StatusCode::OK, "{path}");
    let page = String::from_utf8_lossy(&page).into_owned();
    let mut parts = Vec::new();
    for asset in linked(&page) {
        let (status, body) = get(router, &asset).await;
        assert_eq!(status, StatusCode::OK, "{asset}");
        parts.push((asset, gzip(&body)));
    }
    (parts.iter().map(|(_, n)| n).sum(), parts)
}

#[tokio::test]
async fn each_kind_of_page_stays_within_its_budget() {
    let router = app();
    for (path, budget) in [
        ("/plain", PLAIN_PAGE_GZIP),
        ("/grid", GRID_PAGE_GZIP),
        ("/form", FORM_PAGE_GZIP),
    ] {
        let (total, parts) = page_weight(&router, path).await;
        println!("{path}: {} KB gzip in {} files", total / 1024, parts.len());
        assert!(
            total <= budget,
            "{path} links {total} bytes gzip, over its budget of {budget}: {parts:?}\n  = help: make it smaller, or raise the budget in tests/size_budgets.rs and give the before and after sizes in the commit"
        );
    }
}

#[test]
fn every_module_file_stays_under_the_cap() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let core = ["zeyn.css", "components.css"];
    let mut over = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let code = name.ends_with(".js") || name.ends_with(".css");
        if !code || core.contains(&name.as_str()) {
            continue;
        }
        let size = gzip(&std::fs::read(&path).unwrap());
        if size > MODULE_GZIP {
            over.push((name, size));
        }
    }
    assert!(
        over.is_empty(),
        "these modules are over {MODULE_GZIP} bytes gzip: {over:?}\n  = help: split the module so pages load only the part they use, or raise the cap in tests/size_budgets.rs with the reason and the numbers in the commit"
    );
}
