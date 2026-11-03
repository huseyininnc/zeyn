//! Content components: headings, paragraphs, meta lines, previous and next links, auto grids,
//! lists, avatars, spoilers, letter bars and week schedules.

use zeyn::prelude::*;
use zeyn::{Component, I18n, Render};

fn html(c: &impl Component, locale: &str) -> String {
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

#[params]
pub struct ArticleParams {
    pub id: i64,
}

#[screen(route = "/articles/{id}", public)]
pub fn article(p: ArticleParams) -> Screen {
    Screen::new("Article").body(Paragraph::new(p.id.to_string()))
}

#[test]
fn heading_writes_its_level_and_escapes_text() {
    let h = html(&Heading::h3("Tea <b>").id("tea time!"), "en");
    assert_eq!(h, r#"<h3 class="z-heading z-heading-3" id="teatime">Tea &lt;b&gt;</h3>"#);
    assert!(html(&Heading::h6("x"), "en").starts_with("<h6"));
}

#[test]
fn paragraph_is_escaped_and_takes_muted_and_small() {
    let p = html(&Paragraph::new("a < b").muted().small(), "en");
    assert_eq!(p, r#"<p class="z-paragraph z-muted z-small">a &lt; b</p>"#);
}

#[test]
fn meta_line_hides_separators_from_screen_readers_and_skips_empty_parts() {
    let m = html(
        &MetaLine::new()
            .part("2026")
            .part("")
            .part("5 min <read>"),
        "en",
    );
    assert_eq!(
        m.matches(r#"aria-hidden="true""#)
            .count(),
        1
    );
    assert!(m.contains("5 min &lt;read&gt;"));
    assert!(!m.contains(r#"<span class="z-meta-part"></span>"#));
}

#[test]
fn prev_next_links_carry_rel_and_localized_words() {
    let p = PrevNext::new()
        .prev(article, ArticleParams { id: 1 })
        .prev_title("First <one>")
        .next(article, ArticleParams { id: 3 });
    let en = html(&p, "en");
    assert!(en.contains(r#"rel="prev""#) && en.contains(r#"rel="next""#));
    assert!(en.contains("First &lt;one&gt;"));
    assert!(en.contains(r#"<span class="z-prevnext-arrow" aria-hidden="true">"#));
    assert!(en.contains(r#"aria-label="Previous and next""#));
    let tr = html(&p, "tr");
    assert!(tr.contains("Önceki") && tr.contains("Sonraki"));
    let only = html(&PrevNext::new().next(article, ArticleParams { id: 2 }), "en");
    assert!(!only.contains(r#"rel="prev""#) && only.contains("z-prevnext-gap"));
}

#[test]
fn auto_grid_lets_width_set_the_columns_and_keeps_child_paths() {
    let grid = AutoGrid::new()
        .min(5000)
        .gap(40)
        .add(Paragraph::new("a"))
        .add(Paragraph::new("b"));
    let out = html(&grid, "en");
    assert!(out.contains("--z-autogrid-min:1200px;--z-autogrid-gap:12"), "{out}");
    assert!(out.contains(r#"data-zeyn-node="1""#));
    assert_eq!(grid.children().len(), 2);
    assert_eq!(grid.schema()["type"], "auto_grid");
}

#[test]
fn avatars_show_initials_with_the_name_as_alt_text() {
    let a = html(&zeyn::ui::Avatar::new("ada <b> lovelace"), "en");
    assert!(a.contains(r#"role="img" aria-label="ada &lt;b&gt; lovelace""#), "{a}");
    assert!(a.contains(r#"<span aria-hidden="true">AL</span>"#), "{a}");
    let tr = html(&zeyn::ui::Avatar::new("ilker ışık"), "tr");
    assert!(tr.contains(">İI<"), "Turkish capitals: {tr}");
    let pic = html(
        &zeyn::ui::Avatar::new("Ada")
            .image("/img/ada.png")
            .size(zeyn::ui::AvatarSize::Large),
        "en",
    );
    assert!(pic.contains(r#"src="/img/ada.png" alt="Ada" width="64" height="64""#), "{pic}");
    let outside = html(&zeyn::ui::Avatar::new("Ada").image("https://elsewhere.example/a.png"), "en");
    assert!(!outside.contains("<img") && outside.contains(">A<"), "hosts outside img-src fall back to initials");
    let quiet = html(&zeyn::ui::Avatar::new("Ada").decorative(), "en");
    assert!(quiet.contains(r#"aria-hidden="true""#) && !quiet.contains("aria-label"));
}

#[test]
fn spoilers_hide_their_content_in_a_native_disclosure() {
    let s = zeyn::ui::Spoiler::new("Show <answer>").add(Paragraph::new("Forty two"));
    let out = html(&s, "en");
    assert!(
        out.starts_with(r#"<details class="z-spoiler"><summary class="z-spoiler-label">Show &lt;answer&gt;</summary>"#),
        "{out}"
    );
    assert!(!out.contains("<details open"));
    assert!(out.contains("Forty two") && out.contains(r#"data-zeyn-node="0""#));
    assert_eq!(s.children().len(), 1);
}
