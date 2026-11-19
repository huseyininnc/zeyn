//! The code editor (`CodeEditor`, `Control::Code`): escaped markup, the caps and permission of
//! its completion and check requests, error markers from validation and the page check.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    CodeEditor, CodeError, CodeQuery, Completion, CompletionKind, MAX_CODE_ERRORS,
    MAX_CODE_MESSAGE_CHARS, MAX_CODE_QUERY_BYTES, MAX_COMPLETION_CHARS, MAX_COMPLETIONS,
};
use zeyn::{Component, I18n, Identity, Render};

const HOSTILE: &str = r#"</textarea><script>alert(1)</script>"q' <img src=x onerror=alert(2)>"#;

static CALLS: AtomicUsize = AtomicUsize::new(0);

#[model(validate = check_calc)]
pub struct Calc {
    #[rule(max_len = 2000)]
    pub formula: String,
}

/// Suggests tables for `[`, and far more and far longer items than the caps allow. Counts the
/// calls with a long text, which only the test of the cap sends.
#[action(permission = "reports.design")]
pub async fn complete_fields(q: CodeQuery) -> Result<Vec<Completion>> {
    if q.text.len() > 1000 {
        CALLS.fetch_add(1, Ordering::SeqCst);
    }
    if q.word() == "flood" {
        return Ok((0..500)
            .map(|i| {
                Completion::new(CompletionKind::Field, "x".repeat(1000))
                    .insert(format!("{i}"))
                    .detail("d".repeat(1000))
            })
            .collect());
    }
    Ok(["Orders", "Customers", HOSTILE]
        .iter()
        .filter(|t| t.starts_with(q.word()))
        .map(|t| Completion::new(CompletionKind::Table, *t).detail("table"))
        .collect())
}

/// Marks a few spans, some outside the text, and one message far over the cap.
#[action(permission = "reports.design")]
pub async fn check_formula(q: CodeQuery) -> Result<Vec<CodeError>> {
    if q.text.len() > 1000 {
        CALLS.fetch_add(1, Ordering::SeqCst);
    }
    let mut errors = vec![
        CodeError::new(1, 2, 6, "Unknown table"),
        CodeError::new(40, 99, 50, "Past the end"),
        CodeError::new(0, 0, 0, "m".repeat(5000)),
    ];
    if q.text == "many" {
        errors.extend((0..500).map(|_| CodeError::new(1, 1, 1, "again")));
    }
    Ok(errors)
}

/// Turns away formulas that name an unknown table, with the place of the name.
fn check_calc(c: &Calc, errors: &mut ValidationErrors) {
    if let Some(at) = c.formula.find("[Ordrs") {
        errors.add_at(Calc::formula, 1, at as u32 + 2, 5, "Unknown table Ordrs");
    }
}

#[action(public)]
pub async fn save_calc(c: Valid<Calc>) -> Result<Calc> {
    Ok(c.into_inner())
}

fn form(formula: &str) -> zeyn::SubmitForm<Calc, save_calc> {
    Form::of(Calc {
        formula: formula.into(),
    })
    .field(Calc::formula)
    .control(Calc::formula, Control::Code(CodeLang::Expression))
    .complete(Calc::formula, complete_fields)
    .check(Calc::formula, check_formula)
    .submit("Save", save_calc)
}

#[screen(route = "/calc", public)]
pub fn calc() -> Screen {
    Screen::new("Calc").body(
        Stack::new()
            .add(form("[Orders.Total] * 1.18m"))
            .add(
                CodeEditor::new("sql", CodeLang::Sql)
                    .label("Query")
                    .value(HOSTILE)
                    .complete(complete_fields)
                    .check(check_formula),
            ),
    )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("calc").screen(calc))
        .router()
        .unwrap()
}

async fn send(req: Request<Body>) -> (StatusCode, String) {
    let res = app().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn data(
    perms: &str,
    node: &str,
    key: &str,
    input: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let body = serde_json::json!({ "screen": "calc", "node": node, "key": key, "input": input });
    let req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, text) = send(req).await;
    (
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
    )
}

async fn page(perms: &str) -> String {
    let req = Request::get("/calc")
        .header("x-perms", perms)
        .body(Body::empty())
        .unwrap();
    send(req).await.1
}

#[test]
fn hostile_text_stays_text_in_the_text_area() {
    let mut out = String::new();
    CodeEditor::new("q", CodeLang::Sql)
        .value(HOSTILE)
        .render(&mut out, &Render::new("s", "en", I18n::builtin()));
    assert!(out.contains(
        "&lt;/textarea&gt;&lt;script&gt;alert(1)&lt;/script&gt;&quot;q&#39; &lt;img src=x onerror=alert(2)&gt;</textarea>"
    ));
    assert!(!out.contains("<script") && !out.contains("<img"));
    assert!(
        out.contains(r#"aria-label="SQL""#),
        "the language names a field without a label"
    );
    let mut out = String::new();
    form(HOSTILE).render(&mut out, &Render::new("s", "en", I18n::builtin()));
    assert!(!out.contains("<script") && !out.contains("<img"));
    assert!(out.contains(r#"data-zeyn-code="expression""#));
    assert!(out.contains(r#"maxlength="2000""#));
    assert!(
        out.contains(r#"aria-describedby="s-formula-problems s-formula-error""#),
        "screen readers hear the listed problems and the validation message"
    );
}

#[tokio::test]
async fn the_page_offers_completion_and_checks_only_to_users_who_may_run_them() {
    let allowed = page("reports.design").await;
    assert!(allowed.contains(r#"data-complete="formula:complete""#));
    assert!(allowed.contains(r#"data-check="formula:check""#));
    assert!(allowed.contains(r#"data-complete="complete""#));
    assert!(
        allowed.contains("/_zeyn/codeedit.js"),
        "the page links the editor module"
    );
    assert!(allowed.contains(r#"role="listbox""#));
    let denied = page("other").await;
    assert!(!denied.contains("data-complete=") && !denied.contains("data-check="));
    assert!(denied.contains("z-codeedit-input"));
}

#[tokio::test]
async fn completion_goes_through_the_permission_and_the_caps() {
    let (status, body) = data(
        "reports.design",
        "0.0",
        "formula:complete",
        serde_json::json!({ "text": "1 + [Ord", "cursor": 8 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["label"], "Orders");
    assert_eq!(body[0]["kind"], "table");
    assert_eq!(body.as_array().unwrap().len(), 1);

    let (status, body) = data(
        "reports.design",
        "0.1",
        "complete",
        serde_json::json!({ "text": "flood", "cursor": 5 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), MAX_COMPLETIONS);
    for item in items {
        for k in ["label", "insert", "detail"] {
            assert!(
                item[k]
                    .as_str()
                    .unwrap()
                    .chars()
                    .count()
                    <= MAX_COMPLETION_CHARS
            );
        }
    }

    let (status, _) = data(
        "other",
        "0.0",
        "formula:complete",
        serde_json::json!({ "text": "[Ord", "cursor": 4 }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_text_over_the_cap_is_refused_before_the_action_runs() {
    let (status, _) = data(
        "reports.design",
        "0.1",
        "check",
        serde_json::json!({ "text": "x".repeat(MAX_CODE_QUERY_BYTES + 1), "cursor": 0 }),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    let (status, _) = data(
        "reports.design",
        "0.0",
        "formula:complete",
        serde_json::json!({ "text": "é".repeat(MAX_CODE_QUERY_BYTES / 2 + 1), "cursor": 3 }),
    )
    .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "the cap counts bytes, not characters"
    );
    let (status, body) = data(
        "reports.design",
        "0.0",
        "formula:complete",
        serde_json::json!({ "text": "[Ord", "cursor": 9999 }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a cursor past the end is kept at the end"
    );
    assert_eq!(body[0]["label"], "Orders");
    assert_eq!(
        CALLS.load(Ordering::SeqCst),
        0,
        "no action ran for a text over the cap"
    );
}

#[tokio::test]
async fn completion_and_checks_need_the_csrf_token() {
    let body = serde_json::json!({ "screen": "calc", "node": "0.1", "key": "complete", "input": { "text": "[O", "cursor": 2 } });
    let req = Request::post("/_zeyn/data")
        .header("x-perms", "reports.design")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, _) = send(req).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn check_markers_are_clamped_to_the_text_and_capped() {
    let (status, body) = data(
        "reports.design",
        "0.0",
        "formula:check",
        serde_json::json!({ "text": "[Ordrs] + 1\nsum(", "cursor": 0 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body[0],
        serde_json::json!({ "line": 1, "column": 2, "length": 6, "message": "Unknown table" })
    );
    assert_eq!(
        (&body[1]["line"], &body[1]["column"], &body[1]["length"]),
        (
            &serde_json::json!(2),
            &serde_json::json!(5),
            &serde_json::json!(0)
        ),
        "a marker past the end stays on the last line, at its end"
    );
    assert_eq!(
        (&body[2]["line"], &body[2]["column"]),
        (&serde_json::json!(1), &serde_json::json!(1))
    );
    assert_eq!(
        body[2]["message"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        MAX_CODE_MESSAGE_CHARS
    );
    let (_, body) = data(
        "reports.design",
        "0.1",
        "check",
        serde_json::json!({ "text": "many", "cursor": 0 }),
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), MAX_CODE_ERRORS);
}

#[tokio::test]
async fn validation_errors_carry_the_place_in_the_code() {
    let body = serde_json::json!({ "screen": "calc", "node": "0.0", "values": { "formula": "1 + [Ordrs.Total]" } });
    let req = Request::post("/_zeyn/form")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, text) = send(req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{text}");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let error = &json["errors"][0];
    assert_eq!(error["field"], "formula");
    assert_eq!(
        error["rule"],
        serde_json::json!({ "rule": "code", "value": { "line": 1, "column": 6, "length": 5 } })
    );
}

#[test]
fn completion_and_checks_on_a_field_without_a_code_control_are_a_page_problem() {
    let app = App::new("Test").feature(Feature::new("calc").screen(calc));
    assert!(app.check_page(&calc::run()).is_ok());
    let bad = Screen::new("Bad").body(
        Form::of(Calc::default())
            .field(Calc::formula)
            .complete(Calc::formula, complete_fields),
    );
    let problems = app.check_page(&bad).unwrap_err();
    assert!(problems[0].contains("no code control"), "{problems:?}");
}

#[test]
fn the_query_gives_the_word_before_the_cursor() {
    let q = CodeQuery {
        text: "SUM([Orders.Tot".into(),
        cursor: 15,
    };
    assert_eq!(q.before(), "SUM([Orders.Tot");
    assert_eq!(q.word(), "Orders.Tot");
    let q = CodeQuery {
        text: "Şirket.Ünvan x".into(),
        cursor: 12,
    };
    assert_eq!(
        q.word(),
        "Şirket.Ünvan",
        "the cursor counts characters, not bytes"
    );
}

#[test]
fn expressions_are_highlighted_on_the_server_too() {
    let h = zeyn::ui::highlight(
        CodeLang::Expression,
        r#"Round([Orders.Total] * 1.18m, 2) and "a<b""#,
    );
    assert!(h.starts_with(r#"<span class="z-hl-ty">Round</span>(<span class="z-hl-key">[Orders.Total]</span> <span class="z-hl-op">*</span> <span class="z-hl-num">1.18m</span>"#), "{h}");
    assert!(
        h.contains(
            r#"<span class="z-hl-kw">and</span> <span class="z-hl-str">&quot;a&lt;b&quot;</span>"#
        ),
        "{h}"
    );
}

#[tokio::test]
async fn the_editor_script_builds_no_markup_and_no_inline_styles() {
    let (status, js) = send(
        Request::get("/_zeyn/codeedit.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for sink in [
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "cssText",
        "setAttribute('style'",
        "eval(",
        "new Function",
    ] {
        assert!(!js.contains(sink), "codeedit.js uses {sink}");
    }
}

#[tokio::test]
async fn check_markers_count_characters_not_bytes_or_utf16_units() {
    let (status, body) = data(
        "reports.design",
        "0.1",
        "check",
        serde_json::json!({ "text": "\u{1F600}ab", "cursor": 0 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (&body[0]["column"], &body[0]["length"]),
        (&serde_json::json!(2), &serde_json::json!(2)),
        "after an emoji the span still starts at the second character and ends at the line end"
    );
}

#[test]
fn the_word_before_the_cursor_keeps_combining_letters() {
    let q = CodeQuery {
        text: "[\u{0930}\u{093E}\u{0936}\u{093F}".into(),
        cursor: 5,
    };
    assert_eq!(
        q.word(),
        "\u{0930}\u{093E}\u{0936}\u{093F}",
        "Devanagari vowel signs are letters, as in the browser"
    );
}

#[test]
fn a_leading_newline_of_the_text_survives_the_html_parser() {
    let mut out = String::new();
    CodeEditor::new("q", CodeLang::Sql)
        .value("\nSELECT 1")
        .render(&mut out, &Render::new("s", "en", I18n::builtin()));
    assert!(out.contains(">&#10;\nSELECT 1</textarea>"));
}

#[tokio::test]
async fn the_word_before_the_cursor_is_read_from_a_bounded_slice() {
    let (_, js) = send(
        Request::get("/_zeyn/codeedit.js")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        !js.contains(".slice(0, at).match(WORD_BEFORE)"),
        "matching `[...]*$` against the whole text before the cursor is quadratic in its length"
    );
    assert!(js.contains(".slice(Math.max(0, at - MAX_WORD), at).match(WORD_BEFORE)"));
}
