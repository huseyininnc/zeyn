//! Tooltips, popovers and the busy overlay: markup the runtime needs, escaping, node paths of
//! containers, and that requests from inside them keep every server check.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Render, ScreenDef};

#[model]
pub struct Note {
    #[rule(required, max_len = 40)]
    pub reference: String,
}

#[action(permission = "notes.write")]
pub async fn save_note(note: Valid<Note>) -> Result<Note> {
    Ok(note.into_inner())
}

#[action(permission = "notes.write")]
pub async fn archive_notes(_: ()) -> Result<i64> {
    Ok(3)
}

#[test]
fn field_tooltips_describe_the_input_and_are_escaped() {
    let mut out = String::new();
    Form::of(Note::default())
        .field(Note::reference)
        .tip(Note::reference, "Printed on the <invoice> as \"REF\"")
        .submit("Save", save_note)
        .render(&mut out, &Render::new("n", "tr", I18n::builtin()));
    assert!(
        out.contains(r#"aria-describedby="n-reference-tip n-reference-error""#),
        "{out}"
    );
    assert!(out.contains(r#"<button type="button" class="z-tip-button" aria-label="Reference hakkında" aria-describedby="n-reference-tip" data-zeyn-tip="n-reference-tip">"#));
    assert!(out.contains(r#"<span role="tooltip" class="z-tip" id="n-reference-tip" hidden>Printed on the &lt;invoice&gt; as &quot;REF&quot;</span>"#));
    assert!(!out.contains("<invoice>"));
}

#[test]
fn button_and_toolbar_tooltips_are_capped() {
    let long = "x".repeat(500);
    let mut out = String::new();
    ActionButton::new("Archive", archive_notes, ())
        .tip(Text::from(long.clone()))
        .render(&mut out, &Render::english("n"));
    assert!(out.contains(r#"data-zeyn-tip="n-tip-""#), "{out}");
    assert!(out.contains(&format!(">{}</span>", "x".repeat(zeyn::ui::MAX_TIP))));
    assert!(!out.contains(&"x".repeat(zeyn::ui::MAX_TIP + 1)));
    let html = notes_screen().html(&<notes as ScreenDef>::META);
    assert!(html.contains(r#"<span class="z-tip-host"><button type="button" class="z-button" data-testid="notes.toolbar.new" aria-describedby="notes-tooltip-0" data-zeyn-tip="notes-tooltip-0">"#), "{html}");
}

#[screen(route = "/notes", permission = "notes.read")]
pub fn notes() -> Screen {
    notes_screen()
}

fn notes_screen() -> Screen {
    Screen::new("Notes")
        .toolbar(|t| {
            t.button("New")
                .tip("Opens an empty note")
        })
        .body(
            Stack::new()
                .add(
                    Popover::new("Details")
                        .title("Note rules")
                        .add(
                            Busy::new().delay_ms(9000).add(
                                Form::of(Note::default())
                                    .field(Note::reference)
                                    .submit("Save", save_note),
                            ),
                        ),
                )
                .add(Busy::new().add(ActionButton::new("Archive", archive_notes, ()))),
        )
}

#[test]
fn popovers_and_busy_containers_render_their_children_with_paths() {
    let html = notes_screen().html(&<notes as ScreenDef>::META);
    assert!(
        html.contains(
            r#"aria-haspopup="dialog" aria-expanded="false" aria-controls="notes-popover-0-0""#
        ),
        "{html}"
    );
    assert!(html.contains(r#"<div class="z-popover" id="notes-popover-0-0" role="dialog" aria-labelledby="notes-popover-0-0-title" tabindex="-1" hidden>"#));
    assert!(!html.contains("aria-modal"), "a popover is not modal");
    assert!(
        html.contains(r#"<div class="z-busy" data-zeyn-busy="5000">"#),
        "the delay is capped"
    );
    assert!(
        html.contains(r#"data-zeyn-node="0.0.0.0""#),
        "the form inside keeps a nested path"
    );
    assert!(
        html.contains(r#"<span class="z-busy-text" role="status" data-text="Working"></span>"#)
    );
    let schema = notes_screen().schema(&<notes as ScreenDef>::META);
    let text = schema.to_string();
    assert!(text.contains(r#""type":"popover""#) && text.contains(r#""type":"busy""#));
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn submit(perms: &str, node: &str, values: serde_json::Value) -> StatusCode {
    let app = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(zeyn::Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("notes").screen(notes))
        .router()
        .unwrap();
    let req = Request::post("/_zeyn/form")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(
            serde_json::json!({ "screen": "notes", "node": node, "values": values }).to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let _ = res.into_body().collect().await;
    status
}

#[tokio::test]
async fn forms_inside_popovers_and_busy_containers_keep_their_checks() {
    let ok = serde_json::json!({ "reference": "A-1" });
    assert_eq!(
        submit("notes.read,notes.write", "0.0.0.0", ok.clone()).await,
        StatusCode::OK
    );
    assert_eq!(
        submit("notes.read", "0.0.0.0", ok.clone()).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        submit("notes.write", "0.0.0.0", ok.clone()).await,
        StatusCode::FORBIDDEN,
        "the screen's access"
    );
    assert_eq!(
        submit(
            "notes.read,notes.write",
            "0.0.0.0",
            serde_json::json!({ "reference": "" })
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        submit("notes.read,notes.write", "0.1.0", serde_json::json!({})).await,
        StatusCode::OK
    );
    assert_eq!(
        submit("notes.read,notes.write", "0.0", ok).await,
        StatusCode::NOT_FOUND,
        "the popover is no form"
    );
}
