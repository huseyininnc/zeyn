//! Admin components: DiffView, CodeView and JsonView, barcodes, the inbox and the notification
//! center, Kanban boards and calendars.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    Barcode, CodeLang, CodeView, DiffLayout, DiffView, JsonView, MAX_CODE_LINES, QrCode, QrLevel,
    QrMatrix, code128_symbols, highlight, qr_error_correction, qr_format_bits, qr_version_bits,
};
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

#[test]
fn code_128_packs_digit_runs_and_adds_the_check_symbol() {
    assert_eq!(
        code128_symbols("AB1234567"),
        Some(vec![104, 33, 34, 99, 12, 34, 56, 100, 23, 86, 106])
    );
    assert_eq!(code128_symbols("12"), Some(vec![105, 12, 14, 106]));
    assert_eq!(code128_symbols("1"), Some(vec![104, 17, 18, 106]));
    assert!(code128_symbols("").is_none() && code128_symbols("tab	").is_none());
    assert!(code128_symbols(&"9".repeat(81)).is_none());
}

#[test]
fn a_barcode_has_its_quiet_zones_and_a_text_label() {
    let page = html(&Barcode::code128("PJJ123C"));
    let modules = 11 * 9 + 13;
    assert!(
        page.contains(&format!(r#"viewBox="0 0 {} 1""#, modules + 20)),
        "{page}"
    );
    assert!(
        page.contains(r#"d="M10 0h2v1h-2z"#),
        "the first bar starts after 10 quiet modules"
    );
    assert!(page.contains(r#"role="img" aria-labelledby="s-barcode-""#));
    assert!(
        page.contains(r#"<figcaption class="z-barcode-text" id="s-barcode-">PJJ123C</figcaption>"#)
    );
    assert!(html(&Barcode::code128("Ünlü")).contains("z-barcode-error"));
    let labelled = html(&Barcode::code128("4006381333931").label("Tea, 500 g"));
    assert!(labelled.contains(">Tea, 500 g</figcaption>"));
}

#[test]
fn qr_error_correction_format_and_version_bits_match_the_standard() {
    let data = [
        32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17,
    ];
    assert_eq!(
        qr_error_correction(&data, 10),
        [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]
    );
    assert_eq!(qr_format_bits(QrLevel::Low, 0), 0b111011111000100);
    assert_eq!(qr_format_bits(QrLevel::Quartile, 0), 0b011010101011111);
    assert_eq!(qr_format_bits(QrLevel::High, 0), 0b001011010001001);
    assert_eq!(qr_version_bits(7), 0x07C94);
    assert_eq!(qr_version_bits(40), 0x28C69);
}

#[test]
fn a_qr_code_holds_its_data_in_byte_mode_with_finders_and_the_dark_module() {
    let m = QrMatrix::encode(b"HELLO", QrLevel::Medium).unwrap();
    assert_eq!((m.version(), m.size()), (1, 21));
    let words = m.read_codewords();
    assert_eq!(words.len(), 26);
    assert_eq!(&words[..3], &[0x40, 0x54, 0x84]);
    assert_eq!(words[16..], qr_error_correction(&words[..16], 10)[..]);
    for (x, y) in [(0, 0), (6, 0), (0, 6), (20, 0), (0, 20), (8, 13)] {
        assert!(m.dark(x, y), "({x}, {y})");
    }
    assert!(!m.dark(7, 7) && !m.dark(1, 1));
    let long = QrMatrix::encode("x".repeat(300).as_bytes(), QrLevel::Medium).unwrap();
    assert!(long.version() >= 7 && long.size() == long.version() * 4 + 17);
    assert!(QrMatrix::encode(&[b'a'; 2954], QrLevel::Low).is_none());
}

#[test]
fn a_qr_code_renders_with_four_quiet_modules_and_a_label() {
    let page = html(&QrCode::new("https://example.com/o/42").label("Order 42"));
    let m = QrMatrix::encode(b"https://example.com/o/42", QrLevel::Medium).unwrap();
    let side = m.size() + 8;
    assert!(page.contains(&format!(r#"viewBox="0 0 {side} {side}""#)));
    assert!(page.contains(r#"d="M4 4h7v1h-7z"#), "{page}");
    assert!(page.contains(">Order 42</figcaption>"));
    assert!(html(&QrCode::new("x".repeat(3000))).contains("z-barcode-error"));
}

#[screen(route = "/plain", permission = "admin.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/labels", permission = "admin.read")]
pub fn labels() -> Screen {
    Screen::new("Labels")
        .body(Barcode::code128("SO-1042"))
        .body(CodeView::new(CodeLang::Json, "{}"))
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
                .screen(history)
                .screen(labels),
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
    for name in ["diff.css", "code.css", "barcode.css"] {
        assert!(!text.contains(&format!("/_zeyn/{name}")), "{name}");
    }
    let (status, page) = get("/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("/_zeyn/diff.css?v="));
    let (status, css) = get("/_zeyn/diff.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(css.contains(".z-diff-table"));
    let (_, shown) = get("/labels").await;
    assert!(shown.contains("/_zeyn/barcode.css?v=") && shown.contains("/_zeyn/code.css?v="));
    assert!(
        shown.contains("/_zeyn/share.js?v="),
        "the copy button brings share.js"
    );
    assert!(!shown.contains("/_zeyn/diff.css"));
}
