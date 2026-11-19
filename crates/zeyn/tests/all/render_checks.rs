//! Debug render checks: the problems they find, and no false alarms on what the framework draws.

use axum::body::Body;
use axum::http::{HeaderMap, Request};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::server::render_checks;

#[test]
fn headings_may_go_up_or_one_level_down_but_not_skip() {
    assert!(render_checks("<h1>a</h1><h2>b</h2><h3>c</h3><h2>d</h2><h1>e</h1>").is_empty());
    let p = render_checks("<h1>a</h1><h3>b</h3>");
    assert_eq!(p.len(), 1);
    assert!(p[0].contains("<h3> follows <h1>") && p[0].contains("use <h2> here"));
}

#[test]
fn forms_need_a_way_to_be_sent_unless_the_framework_drives_them() {
    assert!(render_checks(r#"<form><input name="q"><button>Go</button></form>"#).is_empty());
    assert!(render_checks(r#"<form><button type="submit">Go</button></form>"#).is_empty());
    assert!(render_checks(r#"<form data-zeyn-drives="sales"><input name="q"></form>"#).is_empty());
    assert!(render_checks(r#"<form data-zeyn-submit="x"></form>"#).is_empty());
    let p = render_checks(r#"<form><input name="q"><button type="button">Clear</button></form>"#);
    assert_eq!(p.len(), 1);
    assert!(p[0].contains("form without a submit button"));
}

#[test]
fn links_must_not_nest() {
    assert!(render_checks(r#"<a href="/a">a</a><a href="/b">b</a>"#).is_empty());
    let p = render_checks(r#"<a href="/a"><span><a href="/b">b</a></span></a>"#);
    assert!(p[0].contains("link inside a link"));
}

#[model]
pub struct Note {
    #[rule(required)]
    pub title: String,
}

#[action(public)]
pub async fn save_note(n: Valid<Note>) -> Result<()> {
    let _ = n.into_inner();
    Ok(())
}

#[screen(route = "/", public)]
pub fn home() -> Screen {
    Screen::new("Home").body(
        Stack::new()
            .add(Heading::h2("Section"))
            .add(Panel::new("Notes").add(Paragraph::new("Text")))
            .add(Card::new().title("Card"))
            .add(
                Form::of(Note::default())
                    .field(Note::title)
                    .submit("Save", save_note),
            ),
    )
}

#[tokio::test]
async fn a_screen_of_framework_components_raises_no_alarm() {
    let router = App::new("Checks")
        .auth(|_: &HeaderMap| Some(Identity::user("u")))
        .feature(Feature::new("c").screen(home))
        .router()
        .unwrap();
    let res = router
        .oneshot(
            Request::get("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    let main = page
        .split(r#"<main class="z-main" id="z-main">"#)
        .nth(1)
        .and_then(|m| m.split("</main>").next())
        .expect("a main region");
    assert_eq!(render_checks(main), Vec::<String>::new(), "{main}");
}
