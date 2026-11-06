//! Admin components: DiffView, CodeView and JsonView, barcodes, the inbox and the notification
//! center, Kanban boards and calendars.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{CodeLang, CodeView, DiffLayout, DiffView, JsonView, MAX_CODE_LINES, highlight};
use zeyn::{Component, FieldChange, I18n, Identity, Render};

#[choice]
pub enum Stage {
    Draft,
    Approved,
}

#[model]
pub struct Account {
    pub name: String,
    #[mask(last4)]
    pub iban: String,
    pub stage: Stage,
}

fn html(c: &impl Component) -> String {
    let i18n = I18n::builtin();
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", "en", i18n));
    out
}

fn account(iban: &str, stage: Stage) -> Account {
    Account {
        name: "Ada".into(),
        iban: iban.into(),
        stage,
    }
}

#[test]
fn a_diff_shows_only_changed_fields_side_by_side_with_labels_and_choice_labels() {
    let old = account("TR12 3456 7890 1111", Stage::Draft);
    let new = account("TR12 3456 7890 2222", Stage::Approved);
    let page = html(&DiffView::of(&old, &new).title("History"));
    assert!(page.contains(r#"<table class="z-diff-table" aria-labelledby="s-diff--title">"#));
    assert!(page.contains(r#"<th scope="col">Before</th><th scope="col">After</th>"#));
    assert!(page.contains("<del>Draft</del>") && page.contains("<ins>Approved</ins>"));
    assert!(!page.contains(">Ada<"), "unchanged fields are left out");
}

#[test]
fn a_diff_never_shows_a_masked_value_even_from_a_hand_built_change() {
    let old = account("TR12 3456 7890 1111", Stage::Draft);
    let new = account("TR12 3456 7890 2222", Stage::Draft);
    let page = html(&DiffView::of(&old, &new));
    assert!(page.contains("•••• 1111") && page.contains("•••• 2222"));
    assert!(!page.contains("3456"), "{page}");
    let raw = DiffView::changes([FieldChange {
        field: "iban",
        old: Some("TR12 3456 7890 1111".into()),
        new: None,
    }])
    .model::<Account>()
    .layout(DiffLayout::Inline);
    let page = html(&raw);
    assert!(!page.contains("3456"), "{page}");
    assert!(page.contains(r#"<dl class="z-diff-list" aria-label="Changes">"#));
    assert!(page.contains(r#"<span class="z-diff-empty">(empty)</span>"#));
    assert!(
        !raw.schema()
            .to_string()
            .contains("7890")
    );
}

#[test]
fn a_diff_without_changes_says_so_and_escapes_values() {
    assert!(html(&DiffView::changes([])).contains("No changes"));
    let page = html(&DiffView::changes([FieldChange {
        field: "note",
        old: Some("<b>".into()),
        new: Some("x".into()),
    }]));
    assert!(page.contains("<del>&lt;b&gt;</del>") && page.contains(">note<"));
}

#[test]
fn code_is_highlighted_numbered_and_folded_without_script() {
    let src =
        "fn main() {\n    let s = \"<x>\";\n    if s.is_empty() {\n        run();\n    }\n}\n";
    let page = html(&CodeView::new(CodeLang::Rust, src).title("main.rs"));
    assert!(page.contains(r#"<span class="z-hl-kw">fn</span> main() {"#));
    assert!(page.contains("&quot;&lt;x&gt;&quot;") && !page.contains("<x>"));
    assert_eq!(
        page.matches(r#"<details class="z-codeview-fold" open>"#)
            .count(),
        2
    );
    assert_eq!(page.matches("</details>").count(), 2);
    assert_eq!(
        page.matches(r#"<code class="z-codeview-l">"#)
            .count(),
        6
    );
    assert!(
        page.contains(r#"data-zeyn-copy="s-code--body""#) && page.contains("hidden>Copy</button>")
    );
    assert!(page.contains(r#"role="region" tabindex="0" aria-labelledby="s-code--title""#));
    assert!(page.contains("6 lines") && !page.contains("z-codeview-truncated"));
}

#[test]
fn a_block_comment_over_lines_is_closed_and_opened_on_each_line() {
    let page = html(&CodeView::new(
        CodeLang::Sql,
        "/* a\nb */ SELECT 'it''s' FROM t",
    ));
    assert!(
        page.contains(r#"<span class="z-hl-com">/* a</span></code>"#),
        "{page}"
    );
    assert!(page.contains(r#"<code class="z-codeview-l"><span class="z-hl-com">b */</span> <span class="z-hl-kw">SELECT</span> <span class="z-hl-str">&#39;it&#39;&#39;s&#39;</span>"#), "{page}");
}

#[test]
fn long_text_is_cut_with_a_notice_and_json_is_folded_per_object() {
    let text: String = (0..MAX_CODE_LINES + 5)
        .map(|i| format!("line {i}\n"))
        .collect();
    let view = CodeView::new(CodeLang::Plain, text);
    assert!(view.truncated());
    let page = html(&view);
    assert!(page.contains(&format!(
        "Showing the first {MAX_CODE_LINES} of {} lines",
        MAX_CODE_LINES + 5
    )));
    let json = html(&JsonView::new(
        &serde_json::json!({ "a": [1, 2], "b": { "c": true } }),
    ));
    assert_eq!(json.matches("<details").count(), 3);
    assert!(json.contains(r#"<span class="z-hl-kw">true</span>"#));
}

#[test]
fn the_highlighter_knows_rust_json_toml_and_sql() {
    assert!(
        highlight(CodeLang::Toml, "[dependencies]\nzeyn = 1 # x")
            .starts_with(r#"<span class="z-hl-kw">[dependencies]</span>"#)
    );
    assert!(
        highlight(CodeLang::Json, r#"{"k": "v"}"#)
            .contains(r#"<span class="z-hl-key">&quot;k&quot;</span>"#)
    );
    assert!(
        highlight(CodeLang::Sql, "select \"Name\" from t")
            .contains(r#"<span class="z-hl-key">&quot;Name&quot;</span>"#)
    );
    assert!(
        highlight(CodeLang::Rust, "println!(\"x\")")
            .starts_with(r#"<span class="z-hl-mac">println!</span>"#)
    );
}

#[screen(route = "/plain", permission = "admin.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/history", permission = "admin.read")]
pub fn history() -> Screen {
    Screen::new("History").body(DiffView::of(
        &account("A", Stage::Draft),
        &account("A", Stage::Approved),
    ))
}

fn app() -> axum::Router {
    App::new("Admin")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["admin.read"])))
        .feature(
            Feature::new("admin")
                .screen(plain)
                .screen(history),
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

#[tokio::test]
async fn admin_stylesheets_and_scripts_load_only_on_pages_that_use_them() {
    let (_, text) = get("/plain").await;
    for name in ["diff.css"] {
        assert!(!text.contains(&format!("/_zeyn/{name}")), "{name}");
    }
    let (status, page) = get("/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("/_zeyn/diff.css?v="));
    let (status, css) = get("/_zeyn/diff.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(css.contains(".z-diff-table"));
}
