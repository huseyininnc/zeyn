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
