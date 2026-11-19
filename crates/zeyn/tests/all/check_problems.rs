//! Every problem of `App::check` carries a reason, a fix, a docs link and a stable code, and
//! every code names one kind of problem only.

use std::collections::HashMap;
use std::path::Path;
use zeyn::prelude::*;
use zeyn::{DOCS_URL, Problem, problem_json};

#[screen(route = "/a", public)]
async fn first(_: ()) -> Result<Screen> {
    Ok(Screen::new("First"))
}

#[screen(route = "/a", public)]
async fn second(_: ()) -> Result<Screen> {
    Ok(Screen::new("Second"))
}

/// The problems of an app that must fail its check.
fn problems(app: App) -> Vec<String> {
    app.check()
        .expect_err("the app has problems")
}

/// The one problem with `code`, as JSON.
fn find(problems: &[String], code: &str) -> serde_json::Value {
    problems
        .iter()
        .map(|p| problem_json(p))
        .find(|p| p["code"] == code)
        .unwrap_or_else(|| panic!("no {code} in {problems:#?}"))
}

#[test]
fn broken_setups_report_code_reason_fix_and_docs() {
    let cases: Vec<(&str, App)> = vec![
        (
            "Z0101",
            App::new("T")
                .feature(Feature::new("one").screen(first))
                .feature(Feature::new("two").screen(second)),
        ),
        ("Z0111", App::new("T").logo("https://cdn.example/logo.svg")),
        ("Z0112", App::new("T").logo_dark("/static/dark.svg")),
        ("Z0202", App::new("T").csp(|c| c.img_src(["https://*"]))),
        ("Z0203", App::new("T").deny_ips(["not an address"])),
        ("Z0303", App::new("T").site_url("http://shop.example")),
        ("Z0416", App::new("T").palette_shortcut("Ctrl+")),
    ];
    for (code, app) in cases {
        let all = problems(app);
        let p = find(&all, code);
        assert!(
            !p["summary"]
                .as_str()
                .unwrap()
                .is_empty(),
            "{code}"
        );
        assert!(p["why"].as_str().unwrap().len() > 10, "{code}: {p}");
        assert!(p["fix"].as_str().unwrap().len() > 5, "{code}: {p}");
        assert!(
            p["docs"]
                .as_str()
                .unwrap()
                .starts_with(DOCS_URL),
            "{code}: {p}"
        );
    }
}

#[test]
fn the_text_form_keeps_the_summary_first() {
    let all = problems(App::new("T").site_url("http://shop.example"));
    let text = all
        .iter()
        .find(|p| p.contains("Z0303"))
        .unwrap();
    assert!(text.starts_with("site_url `http://shop.example` is not an https origin\n  = why: "));
    assert!(text.contains("\n  = help: pass only the scheme and host"));
    assert!(text.contains(
        "\n  = docs: https://docs.zeynsoft.com/guides/public-pages#search-engines-and-link-previews"
    ));
    assert!(text.ends_with("\n  = code: Z0303"));
}

#[test]
fn describe_lists_problem_details() {
    let d = App::new("T")
        .logo_dark("/static/dark.svg")
        .describe();
    assert_eq!(d["problems"].as_array().unwrap().len(), 1);
    let detail = &d["problem_details"][0];
    assert_eq!(detail["code"], "Z0112");
    assert_eq!(
        detail["summary"],
        "`App::logo_dark` was called without `App::logo`"
    );
    assert_eq!(
        detail["docs"],
        "https://docs.zeynsoft.com/components/shell#the-app-header"
    );
    assert!(
        App::new("T").describe()["problem_details"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn problem_json_reads_the_text_form() {
    let p = Problem::new("Z0101", "summary")
        .why("why")
        .fix("fix")
        .docs("guides/api");
    assert_eq!(problem_json(&p.to_string()), p.json());
    assert_eq!(problem_json("plain")["code"], "");
}

/// Collects `.rs` files under `dir`.
fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e == "rs")
        {
            out.push(path);
        }
    }
}

#[test]
fn every_problem_code_is_used_by_one_kind_of_problem() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut seen: HashMap<String, String> = HashMap::new();
    let marker = "Problem::new(";
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for (i, _) in text.match_indices(marker) {
            let rest = text[i + marker.len()..].trim_start();
            let Some(code) = rest
                .strip_prefix('"')
                .and_then(|r| r.split('"').next())
            else {
                continue;
            };
            if path.ends_with("problem.rs") {
                continue;
            }
            let area: u32 = code[1..3].parse().unwrap_or(0);
            assert!(
                code.len() == 5
                    && code.starts_with('Z')
                    && code[1..]
                        .bytes()
                        .all(|b| b.is_ascii_digit())
                    && (1..=6).contains(&area),
                "{code} in {} is not Z01xx to Z06xx",
                path.display()
            );
            let place = path.display().to_string();
            if let Some(other) = seen.insert(code.to_string(), place.clone()) {
                panic!("{code} is used twice: in {other} and {place}");
            }
        }
    }
    assert!(seen.len() > 80, "found only {} codes", seen.len());
}
