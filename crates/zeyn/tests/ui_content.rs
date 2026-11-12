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

#[params]
pub struct LetterParams {
    pub letter: String,
}

#[screen(route = "/people", public)]
pub fn people(p: LetterParams) -> Screen {
    Screen::new("People").body(Paragraph::new(p.letter))
}

#[test]
fn alpha_index_follows_the_locale_alphabet_and_marks_the_current_letter() {
    let bar = zeyn::ui::AlphaIndex::new(people, |l| LetterParams {
        letter: l.to_string(),
    })
    .current("i")
    .other();
    let tr = html(&bar, "tr");
    let letters: Vec<&str> = tr
        .split(r#"data-zeyn-base-href"#)
        .skip(1)
        .filter_map(|s| s.split('>').nth(1)?.split('<').next())
        .collect();
    assert_eq!(letters.len(), 30, "29 Turkish letters and #: {tr}");
    assert_eq!(&letters[9..13], ["H", "I", "İ", "J"]);
    assert_eq!(letters[29], "#");
    assert_eq!(
        tr.matches(r#"aria-current="page""#)
            .count(),
        1
    );
    assert!(
        tr.contains(r#"aria-current="page">İ<"#),
        "Turkish i is the dotted capital: {tr}"
    );
    assert!(
        tr.contains(r#"aria-label="Harfe göre gözat""#)
            && tr.contains(r#"aria-label="Diğer karakterler""#)
    );
    let en = html(&bar, "en");
    assert!(en.contains(r#"aria-current="page">I<"#), "{en}");
    assert!(en.contains("/_zeyn/route/people?params="), "{en}");
    assert!(
        !html(
            &zeyn::ui::AlphaIndex::new(people, |l| LetterParams { letter: l.into() }),
            "zh"
        )
        .contains("#<")
    );
}

#[test]
fn heading_writes_its_level_and_escapes_text() {
    let h = html(&Heading::h3("Tea <b>").id("tea time!"), "en");
    assert_eq!(
        h,
        r#"<h3 class="z-heading z-heading-3" id="teatime">Tea &lt;b&gt;</h3>"#
    );
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
    assert!(!m.contains(r#"<bdi class="z-meta-part"></bdi>"#));
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
    let only = html(
        &PrevNext::new().next(article, ArticleParams { id: 2 }),
        "en",
    );
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
    assert!(
        out.contains("--z-autogrid-min:1200px;--z-autogrid-gap:12"),
        "{out}"
    );
    assert!(out.contains(r#"data-zeyn-node="1""#));
    assert_eq!(grid.children().len(), 2);
    assert_eq!(grid.schema()["type"], "auto_grid");
}

#[test]
fn avatars_show_initials_with_the_name_as_alt_text() {
    let a = html(&zeyn::ui::Avatar::new("ada <b> lovelace"), "en");
    assert!(
        a.contains(r#"role="img" aria-label="ada &lt;b&gt; lovelace""#),
        "{a}"
    );
    assert!(a.contains(r#"<span aria-hidden="true">AL</span>"#), "{a}");
    let tr = html(&zeyn::ui::Avatar::new("ilker ışık"), "tr");
    assert!(tr.contains(">İI<"), "Turkish capitals: {tr}");
    let pic = html(
        &zeyn::ui::Avatar::new("Ada")
            .image("/img/ada.png")
            .size(zeyn::ui::AvatarSize::Large),
        "en",
    );
    assert!(
        pic.contains(r#"src="/img/ada.png" alt="Ada" width="64" height="64""#),
        "{pic}"
    );
    let outside = html(
        &zeyn::ui::Avatar::new("Ada").image("https://elsewhere.example/a.png"),
        "en",
    );
    assert!(
        !outside.contains("<img") && outside.contains(">A<"),
        "hosts outside img-src fall back to initials"
    );
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

#[model]
pub struct Session {
    pub name: String,
    pub day: Option<Date>,
}

#[action(permission = "week.read")]
pub async fn week_sessions(q: DataQuery) -> Result<Page<Session>> {
    let at = |d: u8| Date::new(2026, 11, d);
    let rows = vec![
        Session {
            name: "Yoga <am>".into(),
            day: at(2),
        },
        Session {
            name: "Cycling".into(),
            day: at(4),
        },
        Session {
            name: "Swim".into(),
            day: at(9),
        },
        Session {
            name: "Unplanned".into(),
            day: None,
        },
    ];
    Ok(q.apply(rows))
}

#[screen(route = "/week", public)]
pub fn week() -> Screen {
    Screen::new("Week").body(
        WeekSchedule::of(week_sessions)
            .title("This week")
            .day(|s: &Session| s.day)
            .item(|s: &Session| zeyn::ui::ListItem::new(s.name.clone())),
    )
}

async fn week_page(locale: &str, perms: &str) -> (u16, String) {
    use tower::ServiceExt;
    let router = App::new("Test")
        .i18n(
            I18n::new("en")
                .catalog("tr", "{}")
                .unwrap(),
        )
        .fixed_now(DateTime::parse("2026-11-04T10:00:00Z").unwrap())
        .auth(|h: &axum::http::HeaderMap| {
            let perms = h.get("x-perms")?.to_str().ok()?;
            let perms: Vec<String> = perms
                .split(',')
                .filter(|p| !p.is_empty())
                .map(String::from)
                .collect();
            Some(zeyn::Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("week").screen(week))
        .router()
        .unwrap();
    let req = axum::http::Request::get("/week")
        .header("x-perms", perms)
        .header("cookie", format!("zeyn_locale={locale}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    let status = res.status().as_u16();
    let body = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn week_schedules_place_rows_by_day_in_the_locales_week_and_mark_today() {
    let (status, tr) = week_page("tr", "week.read").await;
    assert_eq!(status, 200, "{tr}");
    let days: Vec<&str> = tr
        .match_indices(r#"class="z-week-name""#)
        .map(|(i, _)| &tr[i..i + 120])
        .collect();
    assert_eq!(days.len(), 7, "{tr}");
    assert!(
        days[0].contains("Pazartesi") && days[6].contains("Pazar<"),
        "Monday first in Turkish: {days:?}"
    );
    assert!(tr.contains(r#"<section class="z-week-day z-week-today""#));
    assert!(
        days[2].contains("Bugün"),
        "4 November 2026 is a Wednesday: {days:?}"
    );
    let monday = &tr[tr.find("Pazartesi").unwrap()..tr.find("Salı").unwrap()];
    assert!(
        monday.contains("Yoga &lt;am&gt;") && monday.contains("Swim"),
        "rows go by weekday: {monday}"
    );
    assert!(!tr.contains("Unplanned"), "rows without a day are left out");
    assert_eq!(tr.matches("Plan yok").count(), 5);
    let (_, en) = week_page("en", "week.read").await;
    let first = en
        .find(r#"class="z-week-name""#)
        .unwrap();
    assert!(
        en[first..first + 120].contains("Sunday"),
        "Sunday first in English: {en}"
    );
    let (_, denied) = week_page("en", "").await;
    assert!(
        !denied.contains("Cycling"),
        "reading needs the action's permission"
    );
}

#[test]
fn link_shortcuts_carry_aria_keyshortcuts_and_a_hint_and_browser_keys_are_not_bound() {
    let p = PrevNext::new()
        .prev(article, ArticleParams { id: 1 })
        .shortcut("alt+shift+b")
        .next(article, ArticleParams { id: 3 })
        .shortcut("Ctrl+W");
    let out = html(&p, "en");
    assert!(
        out.contains(r#"rel="prev" data-zeyn-base-href aria-keyshortcuts="Alt+Shift+B">"#),
        "{out}"
    );
    assert!(out.contains(r#"<kbd class="z-kbd" aria-hidden="true">Alt+Shift+B</kbd>"#));
    assert!(
        out.contains(r#"rel="next" data-zeyn-base-href>"#),
        "a shortcut the browser keeps is not bound: {out}"
    );
    let chips = Chips::links("Topics")
        .link("One", article, ArticleParams { id: 1 })
        .link("Two", article, ArticleParams { id: 2 })
        .shortcut("Ctrl+Alt+2");
    let out = html(&chips, "en");
    assert!(out.contains(r#"data-zeyn-base-href>One</a>"#), "{out}");
    assert!(
        out.contains(r#"aria-keyshortcuts="Control+Alt+2">Two<kbd class="z-kbd" aria-hidden="true">Ctrl+Alt+2</kbd></a>"#),
        "{out}"
    );
    assert_eq!(chips.shortcuts(), ["Ctrl+Alt+2"]);
}

#[screen(route = "/linked", public)]
pub fn linked() -> Screen {
    Screen::new("Linked").body(
        Stack::new().add(
            PrevNext::new()
                .prev(article, ArticleParams { id: 1 })
                .shortcut("Alt+Shift+P")
                .next(article, ArticleParams { id: 2 })
                .shortcut("Shift+N"),
        ),
    )
}

#[screen(route = "/other", public)]
pub fn other() -> Screen {
    Screen::new("Other").body(
        Chips::links("Topics")
            .link("One", article, ArticleParams { id: 1 })
            .shortcut("Ctrl+Alt+K")
            .link("Two", article, ArticleParams { id: 2 })
            .shortcut("Ctrl+T"),
    )
}

#[test]
fn app_check_reports_link_shortcut_clashes_in_the_header_and_check_page_on_a_screen() {
    let app = App::new("Links").feature(
        Feature::new("site")
            .screen(article)
            .screen(linked)
            .screen(other)
            .command(
                Command::navigate("go.k", "Go", article, ArticleParams { id: 9 })
                    .shortcut("Ctrl+Alt+K"),
            )
            .shell(Shell::site().header(|h| {
                h.bottom(
                    Chips::links("Top")
                        .link("Top", article, ArticleParams { id: 1 })
                        .shortcut("Ctrl+Alt+K"),
                )
            })),
    );
    let problems = app.check().unwrap_err();
    assert!(
        problems.iter().any(|p| p.contains("`Control+Alt+K` in the bottom slot of the site header of feature `site` (node 3.0) is also used by command `go.k`")),
        "{problems:?}"
    );
    let page = app
        .check_page(&linked::run())
        .unwrap_err()
        .join("\n");
    assert!(
        page.contains("`Alt+Shift+P` at node 0.0 is also used by the command palette"),
        "{page}"
    );
    assert!(
        page.contains("`Shift+N` at node 0.0 is a single character key"),
        "{page}"
    );
    let page = app
        .check_page(&other::run())
        .unwrap_err()
        .join("\n");
    assert!(
        page.contains("`Control+T` at node 0 is kept by the browser"),
        "{page}"
    );
    assert!(
        page.contains("`Control+Alt+K` at node 0 is also used by command `go.k`"),
        "{page}"
    );
    assert!(
        page.contains(
            "`Control+Alt+K` in the site header (node 3.0) is also used by command `go.k`"
        ),
        "{page}"
    );
    let clean = App::new("Clean").feature(Feature::new("c").screen(linked));
    assert!(
        clean
            .check_page(&article::run(ArticleParams { id: 1 }))
            .is_ok()
    );
}

#[params]
pub struct PageParams {
    pub page: u32,
}

#[screen(route = "/blog", public)]
pub fn blog(p: PageParams) -> Screen {
    Screen::new("Blog").body(Paragraph::new(p.page.to_string()))
}

#[test]
fn avatar_groups_show_the_first_avatars_and_count_the_rest() {
    let group = [
        "Ada Lovelace",
        "Alan Turing",
        "Grace Hopper",
        "Edsger Dijkstra",
    ]
    .into_iter()
    .fold(zeyn::ui::AvatarGroup::new("Team <one>"), |g, name| {
        g.add(zeyn::ui::Avatar::new(name))
    })
    .max(2)
    .size(zeyn::ui::AvatarSize::Small);
    let out = html(&group, "en");
    assert!(
        out.starts_with(
            r#"<div class="z-avatar-group" role="group" aria-label="Team &lt;one&gt;">"#
        ),
        "{out}"
    );
    assert_eq!(
        out.matches("z-avatar-32").count(),
        3,
        "two people and the count: {out}"
    );
    assert!(out.contains(r#"aria-label="Ada Lovelace""#) && !out.contains("Grace Hopper"));
    assert!(
        out.contains(
            r#"z-avatar-more" role="img" aria-label="2 more"><span aria-hidden="true">+2</span>"#
        ),
        "{out}"
    );
    assert!(html(&group, "tr").contains(r#"aria-label="2 kişi daha""#));
    let all = html(
        &zeyn::ui::AvatarGroup::new("Team").add(zeyn::ui::Avatar::new("Ada")),
        "en",
    );
    assert!(
        !all.contains("z-avatar-more"),
        "nothing left to count: {all}"
    );
    assert_eq!(
        group.schema()["avatars"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn dividers_are_separators_with_an_optional_label() {
    assert_eq!(
        html(&zeyn::ui::Divider::new(), "en"),
        r#"<hr class="z-divider">"#
    );
    let named = html(&zeyn::ui::Divider::new().label("or <b>"), "en");
    assert!(
        named.contains(r#"role="separator" aria-label="or &lt;b&gt;""#)
            && named.contains(
                r#"<span class="z-divider-label" aria-hidden="true">or &lt;b&gt;</span>"#
            ),
        "{named}"
    );
    let vertical = html(
        &zeyn::ui::Divider::new()
            .label("or")
            .vertical(),
        "en",
    );
    assert!(
        vertical.contains(r#"aria-orientation="vertical""#) && !vertical.contains(">or<"),
        "{vertical}"
    );
}

#[test]
fn kbd_writes_each_key_of_a_combination() {
    let out = html(&zeyn::ui::Kbd::new("Ctrl + Shift+<P>"), "en");
    assert_eq!(
        out,
        r#"<kbd class="z-keys"><kbd>Ctrl</kbd><span class="z-keys-plus">+</span><kbd>Shift</kbd><span class="z-keys-plus">+</span><kbd>&lt;P&gt;</kbd></kbd>"#
    );
    let plus = html(&zeyn::ui::Kbd::keys(["Ctrl", "+"]), "en");
    assert!(plus.ends_with("<kbd>+</kbd></kbd>"), "{plus}");
    let many = zeyn::ui::Kbd::keys(["1", "2", "3", "4", "5", "6", "7", "8"]);
    assert_eq!(
        many.schema()["keys"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_KBD_KEYS
    );
}

#[test]
fn quotes_are_blockquotes_with_their_author_and_source() {
    let q = zeyn::ui::Quote::new("Less is <more>.")
        .author("Ada")
        .source("Notes");
    let out = html(&q, "en");
    assert!(
        out.starts_with(
            r#"<figure class="z-quote"><blockquote class="z-quote-text"><p>Less is &lt;more&gt;.</p></blockquote>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<span class="z-quote-author">Ada</span><cite class="z-quote-source">Notes</cite>"#
        ),
        "{out}"
    );
    let bare = html(&zeyn::ui::Quote::new("Alone"), "en");
    assert!(!bare.contains("figcaption"), "{bare}");
}

#[test]
fn accordions_are_native_disclosures_that_can_keep_one_section_open() {
    let free = zeyn::ui::Accordion::new()
        .section("First <1>", Paragraph::new("One"))
        .open()
        .section("Second", Paragraph::new("Two"))
        .open();
    let out = html(&free, "en");
    assert_eq!(out.matches("<details").count(), 2);
    assert_eq!(out.matches(" open>").count(), 2, "both start open: {out}");
    assert!(!out.contains("name="), "sections are independent: {out}");
    assert!(out.contains(r#"<summary class="z-accordion-title">First &lt;1&gt;</summary>"#));
    assert!(out.contains(r#"data-zeyn-node="0""#) && out.contains(r#"data-zeyn-node="1""#));
    assert_eq!(free.children().len(), 2);

    let single = zeyn::ui::Accordion::new()
        .section("First", Paragraph::new("One"))
        .section("Second", Paragraph::new("Two"))
        .open()
        .section("Third", Paragraph::new("Three"))
        .open()
        .single();
    let out = html(&single, "en");
    assert_eq!(
        out.matches(r#" name="s-accordion-""#)
            .count(),
        3,
        "the sections share a name, so the browser closes the others: {out}"
    );
    assert_eq!(
        out.matches(" open>").count(),
        1,
        "only one starts open: {out}"
    );
    assert_eq!(single.schema()["sections"][1]["open"], true);
    assert_eq!(single.schema()["sections"][2]["open"], false);

    let capped = (0..zeyn::ui::MAX_ACCORDION_SECTIONS + 5)
        .fold(zeyn::ui::Accordion::new(), |a, i| {
            a.section(i.to_string(), Paragraph::new("x"))
        });
    assert_eq!(capped.children().len(), zeyn::ui::MAX_ACCORDION_SECTIONS);
}

#[test]
fn steppers_mark_done_current_and_coming_steps() {
    let s = zeyn::ui::Stepper::new("Order <progress>")
        .step("Placed")
        .step("Packed")
        .description("At the <warehouse>")
        .step("Shipped")
        .current(1);
    let out = html(&s, "en");
    assert!(
        out.starts_with(r#"<ol class="z-stepper" aria-label="Order &lt;progress&gt;">"#),
        "{out}"
    );
    assert_eq!(
        out.matches(r#"aria-current="step""#)
            .count(),
        1
    );
    assert!(
        out.contains(
            r#"<li class="z-stage z-stage-done"><span class="z-stage-mark" aria-hidden="true">&#10003;</span>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"Placed<span class="z-sr-only"> (completed)</span>"#),
        "the state is said, not only colored: {out}"
    );
    assert!(out.contains(
        r#"<li class="z-stage z-stage-current" aria-current="step"><span class="z-stage-mark" aria-hidden="true">2</span>"#
    ));
    assert!(out.contains(r#"<span class="z-stage-desc">At the &lt;warehouse&gt;</span>"#));
    assert!(out.contains(r#"<li class="z-stage z-stage-todo">"#));
    assert!(html(&s, "tr").contains("(tamamlandı)"));

    let done = zeyn::ui::Stepper::new("Order")
        .step("Placed")
        .step("Shipped")
        .finished()
        .vertical();
    let out = html(&done, "en");
    assert!(out.contains("z-stepper-vertical") && !out.contains("aria-current"));
    assert_eq!(out.matches("z-stage-done").count(), 2);

    let capped = (0..zeyn::ui::MAX_STEPS + 3)
        .fold(zeyn::ui::Stepper::new("Many"), |s, i| s.step(i.to_string()));
    assert_eq!(
        capped.schema()["steps"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_STEPS
    );
}

#[test]
fn page_windows_keep_the_ends_and_never_hide_a_single_page() {
    use zeyn::ui::page_window;
    let numbers = |pages: Vec<Option<u32>>| {
        pages
            .iter()
            .map(|p| p.map_or("..".to_string(), |n| n.to_string()))
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(numbers(page_window(1, 1, 1)), "1");
    assert_eq!(numbers(page_window(1, 10, 1)), "1 2 .. 10");
    assert_eq!(numbers(page_window(4, 10, 1)), "1 2 3 4 5 .. 10");
    assert_eq!(numbers(page_window(5, 10, 1)), "1 .. 4 5 6 .. 10");
    assert_eq!(numbers(page_window(7, 10, 1)), "1 .. 6 7 8 9 10");
    assert_eq!(numbers(page_window(10, 10, 2)), "1 .. 8 9 10");
    assert_eq!(numbers(page_window(99, 10, 0)), "1 .. 10");
    assert_eq!(numbers(page_window(0, 3, 0)), "1 2 3");
    assert!(page_window(1, 0, 1).is_empty());
}

#[test]
fn pagination_links_pages_of_a_screen_and_marks_the_current_one() {
    let nav = |current: u32| {
        zeyn::ui::Pagination::new(blog, |page| PageParams { page })
            .pages(9)
            .current(current)
    };
    let out = html(&nav(5), "en");
    assert!(
        out.starts_with(r#"<nav class="z-pagination" aria-label="Pagination">"#),
        "{out}"
    );
    assert_eq!(
        out.matches(r#"aria-current="page""#)
            .count(),
        1
    );
    assert!(
        out.contains(r#"aria-label="Page 5" aria-current="page">5</a>"#),
        "{out}"
    );
    assert!(out.contains(r#"rel="prev">Previous</a>"#) && out.contains(r#"rel="next">Next</a>"#));
    assert_eq!(out.matches("z-pagination-gap").count(), 2);

    let first = html(&nav(0), "en");
    assert!(
        first.contains(
            r#"<span class="z-pagelink z-pagelink-step" aria-disabled="true">Previous</span>"#
        ),
        "no link before the first page, and page 0 is page 1: {first}"
    );
    assert!(first.contains(r#"aria-label="Page 1" aria-current="page""#));
    let last = html(&nav(500), "tr");
    assert!(
        last.contains(r#"aria-disabled="true">Sonraki</span>"#)
            && last.contains(r#"aria-label="Sayfa 9" aria-current="page""#),
        "a page past the end is the last page: {last}"
    );
    assert!(last.contains(r#"aria-label="Sayfalama""#));

    let one = zeyn::ui::Pagination::new(blog, |page| PageParams { page }).pages(1);
    assert_eq!(html(&one, "en"), "", "one page needs no links");
    assert_eq!(nav(500).schema()["current"], 9);
}

#[test]
fn progress_rings_are_progressbars_named_by_their_label() {
    let ring = zeyn::ui::ProgressRing::new("Storage <used>", 150, 100)
        .tone(Tone::Warning)
        .size(4);
    let out = html(&ring, "en");
    assert!(
        out.starts_with(r#"<div class="z-ring z-tone-warning" style="--z-ring-size:32px">"#),
        "the size is kept within its limits: {out}"
    );
    assert!(
        out.contains(
            r#"role="progressbar" aria-labelledby="s-ring-" aria-valuemin="0" aria-valuemax="100" aria-valuenow="100" aria-valuetext="100%""#
        ),
        "the value never passes the maximum: {out}"
    );
    assert!(out.contains(r#"stroke-dasharray="100 100""#));
    assert!(out.contains(r#"<span class="z-ring-label" id="s-ring-">Storage &lt;used&gt;</span>"#));
    let empty = html(&zeyn::ui::ProgressRing::new("None", 0, 0), "en");
    assert!(
        empty.contains(r#"aria-valuemax="1" aria-valuenow="0""#),
        "{empty}"
    );
}

#[test]
fn copy_buttons_carry_their_text_and_start_hidden() {
    let out = html(&zeyn::ui::CopyButton::new("a<b>&c"), "en");
    assert!(
        out.contains(r#"<code class="z-copy-text" id="s-copy-" hidden>a&lt;b&gt;&amp;c</code>"#),
        "{out}"
    );
    assert!(
        out.contains(
            r#"data-zeyn-copy="s-copy-" data-copied="Copied to the clipboard" data-failed="Could not copy" data-testid="s.copy" hidden>Copy</button>"#
        ),
        "{out}"
    );
    let shown = html(
        &zeyn::ui::CopyButton::new("ZX-1")
            .shown()
            .label("Copy the code"),
        "en",
    );
    assert!(
        shown.contains(r#"id="s-copy-">ZX-1</code>"#) && shown.contains(">Copy the code</button>"),
        "{shown}"
    );
    let long = zeyn::ui::CopyButton::new("ş".repeat(zeyn::ui::MAX_COPY_BYTES));
    let kept = long.schema()["text"]
        .as_str()
        .unwrap()
        .len();
    assert_eq!(
        kept,
        zeyn::ui::MAX_COPY_BYTES,
        "cut at a character boundary"
    );
}

#[test]
fn heatmaps_are_tables_of_days_in_the_locales_week_with_the_number_as_text() {
    let end = Date::new(2026, 11, 12).unwrap();
    let map = zeyn::ui::Heatmap::ending("Orders <a day>", end)
        .weeks(2)
        .value(end, 8)
        .value(Date::new(2026, 11, 9).unwrap(), 2)
        .value(Date::new(2026, 1, 1).unwrap(), 900)
        .tone(Tone::Success);
    let en = html(&map, "en");
    assert!(
        en.starts_with(r#"<div class="z-heat z-tone-success"><div class="z-heat-scroll" role="group" aria-labelledby="s-heat-" tabindex="0">"#),
        "{en}"
    );
    assert!(
        en.contains(r#"<caption class="z-heat-title" id="s-heat-">Orders &lt;a day&gt;</caption>"#)
    );
    assert_eq!(en.matches("<tr>").count(), 7);
    let cells = en
        .split("</tbody>")
        .next()
        .unwrap()
        .matches("z-heat-cell")
        .count();
    let off = en.matches("z-heat-off").count();
    assert_eq!(cells + off, 14, "two weeks of seven days: {en}");
    assert!(off > 0, "days after the end stay empty");
    assert!(
        en.contains(r#"<td class="z-heat-cell z-heat-4" title="11/12/2026: 8"><span class="z-sr-only">11/12/2026: 8</span></td>"#),
        "the largest shown number is the darkest, a number outside the weeks does not count: {en}"
    );
    assert!(en.contains(r#"z-heat-1" title="11/9/2026: 2""#), "{en}");
    assert!(
        en.contains(r#"aria-hidden="true"><span>Less</span>"#) && en.contains("<span>More</span>")
    );

    let tr = html(&map, "tr");
    let first_row = tr.split("<tr>").nth(1).unwrap();
    assert!(
        first_row.starts_with(r#"<th class="z-heat-day" scope="row">Pazartesi</th>"#),
        "the Turkish week starts on Monday: {first_row}"
    );
    assert!(tr.contains("12.11.2026: 8") && tr.contains("<span>Az</span>"));

    let year = zeyn::ui::Heatmap::ending("Year", end).weeks(200);
    assert_eq!(year.schema()["weeks"], zeyn::ui::MAX_HEATMAP_WEEKS);
    assert_eq!(zeyn::ui::heat_level(u32::MAX, u32::MAX), 4);
    assert_eq!(zeyn::ui::heat_level(1, u32::MAX), 1);
}

#[test]
fn tables_name_their_columns_and_rows_and_pad_short_rows() {
    let t = zeyn::ui::Table::new("Plans <2026>")
        .head(["Plan", "Price"])
        .row(["Free <b>", "0"])
        .row(["Team"])
        .row(["Extra", "9", "dropped"])
        .row_headers()
        .numeric(1)
        .striped();
    let out = html(&t, "en");
    assert!(
        out.starts_with(r#"<div class="z-table-scroll" role="group" aria-labelledby="s-table-" tabindex="0"><table class="z-table z-table-striped"><caption class="z-table-caption" id="s-table-">Plans &lt;2026&gt;</caption>"#),
        "{out}"
    );
    assert!(out.contains(
        r#"<thead><tr><th scope="col">Plan</th><th scope="col" class="z-table-num">Price</th></tr></thead>"#
    ));
    assert!(
        out.contains(
            r#"<tr><th scope="row">Free &lt;b&gt;</th><td class="z-table-num">0</td></tr>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"<tr><th scope="row">Team</th><td class="z-table-num"></td></tr>"#),
        "a short row is filled: {out}"
    );
    assert!(!out.contains("dropped"), "a long row is cut at the head");

    let bare = html(&zeyn::ui::Table::new("Bare").row(["a", "b", "c"]), "en");
    assert!(
        !bare.contains("<thead>") && bare.contains("<tr><td>a</td><td>b</td><td>c</td></tr>"),
        "{bare}"
    );
    let capped = (0..zeyn::ui::MAX_TABLE_ROWS + 9)
        .fold(zeyn::ui::Table::new("Many"), |t, i| t.row([i.to_string()]));
    assert_eq!(
        capped.schema()["rows"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_TABLE_ROWS
    );
    let wide = zeyn::ui::Table::new("Wide").head((0..40).map(|i| i.to_string()));
    assert_eq!(
        wide.schema()["head"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_TABLE_COLUMNS
    );
}

#[test]
fn indicators_hide_the_mark_and_say_the_label() {
    let count = zeyn::ui::Indicator::on(zeyn::ui::Avatar::new("Ada"))
        .count(3)
        .label("3 unread <messages>");
    let out = html(&count, "en");
    assert!(out.starts_with(r#"<div class="z-indicator"><div class="z-node" data-zeyn-node="0">"#));
    assert!(
        out.contains(
            r#"<span class="z-indicator-mark z-indicator-count z-tone-danger" aria-hidden="true">3</span><span class="z-sr-only">3 unread &lt;messages&gt;</span>"#
        ),
        "{out}"
    );
    assert_eq!(count.children().len(), 1);

    let many = html(
        &zeyn::ui::Indicator::on(Badge::new("Inbox")).count(250),
        "en",
    );
    assert!(
        many.contains(r#"aria-hidden="true">99+</span><span class="z-sr-only">99+</span>"#),
        "{many}"
    );
    let none = html(
        &zeyn::ui::Indicator::on(Badge::new("Inbox"))
            .count(0)
            .label("nothing"),
        "en",
    );
    assert!(
        !none.contains("z-indicator-mark") && !none.contains("nothing"),
        "zero shows and says nothing: {none}"
    );
    let dot = html(
        &zeyn::ui::Indicator::on(Badge::new("Ada"))
            .dot()
            .tone(Tone::Success),
        "en",
    );
    assert!(
        dot.contains(r#"z-indicator-dot z-tone-success" aria-hidden="true"></span></div>"#),
        "a dot without a label is decoration: {dot}"
    );
}

#[test]
fn meters_say_their_state_in_words_and_color_it_by_direction() {
    use zeyn::ui::{Better, Meter, MeterState};
    let disk = Meter::new("Disk <used>", 4120, 0, 5000)
        .high(4000)
        .better(Better::Lower)
        .unit("GB");
    assert_eq!(disk.state(), MeterState::High);
    let out = html(&disk, "en");
    assert!(
        out.starts_with(r#"<div class="z-meter z-tone-danger">"#),
        "{out}"
    );
    assert!(out.contains(r#"<span id="s-meter-">Disk &lt;used&gt;</span>"#));
    assert!(
        out.contains(
            r#"<span class="z-meter-value">4,120 GB<span class="z-meter-state">high</span></span>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(
            r#"role="meter" aria-labelledby="s-meter-" aria-valuemin="0" aria-valuemax="5000" aria-valuenow="4120" aria-valuetext="4,120 GB, high""#
        ),
        "{out}"
    );
    assert!(out.contains(r#"style="--z-meter:82%""#));
    assert!(html(&disk, "tr").contains(r#"aria-valuetext="4.120 GB, yüksek""#));

    let tone = |m: Meter| html(&m, "en")[..40].to_string();
    assert!(
        tone(
            Meter::new("Stock", 50, 0, 100)
                .low(20)
                .high(80)
        )
        .contains("z-tone-success")
    );
    assert!(
        tone(
            Meter::new("Stock", 10, 0, 100)
                .low(20)
                .high(80)
        )
        .contains("z-tone-warning")
    );
    assert!(
        tone(
            Meter::new("Stock", 10, 0, 100)
                .low(20)
                .better(Better::Higher)
        )
        .contains("z-tone-danger")
    );
    assert!(
        tone(
            Meter::new("Stock", 50, 0, 100)
                .low(20)
                .high(80)
                .better(Better::Higher)
        )
        .contains("z-tone-warning")
    );
    assert!(
        tone(
            Meter::new("Stock", 90, 0, 100)
                .low(20)
                .high(80)
                .better(Better::Higher)
        )
        .contains("z-tone-success")
    );

    let wild = html(&Meter::new("Wild", i64::MAX, i64::MIN, i64::MAX), "en");
    assert!(wild.contains(r#"--z-meter:100%"#), "{wild}");
    let flat = html(&Meter::new("Flat", 9, 5, 5), "en");
    assert!(
        flat.contains(r#"aria-valuemin="5" aria-valuemax="6" aria-valuenow="6""#),
        "an empty range gets one step and the value stays inside: {flat}"
    );
}

#[test]
fn highlights_mark_every_word_of_the_query_and_escape_both() {
    use zeyn::ui::{Highlight, highlight_ranges};
    let out = html(&Highlight::new("Green <tea> & TEA cups", "tea <"), "en");
    assert_eq!(
        out,
        r#"<span class="z-highlight">Green <mark>&lt;</mark><mark>tea</mark>&gt; &amp; <mark>TEA</mark> cups</span>"#
    );
    assert_eq!(
        html(&Highlight::new("a<b", ""), "en"),
        r#"<span class="z-highlight">a&lt;b</span>"#
    );
    assert_eq!(highlight_ranges("teapot tea", "tea teapot"), [0..6, 7..10]);
    assert_eq!(highlight_ranges("ŞEKER şeker", "şeker"), [0..6, 7..13]);
    assert_eq!(
        highlight_ranges("İstanbul", "i"),
        [],
        "a match never ends inside a character"
    );
    let words: Vec<String> = ('a'..='z')
        .map(|c| format!("{c}q"))
        .collect();
    let text = words.join(" ");
    assert_eq!(
        highlight_ranges(&text, &text).len(),
        zeyn::ui::MAX_HIGHLIGHT_TERMS,
        "only the first words of a long query are looked for"
    );
    assert!(highlight_ranges("aaaa", &"a".repeat(65)).is_empty());
}

#[test]
fn tocs_link_headings_by_their_cleaned_anchors_and_nest_sub_entries() {
    let toc = zeyn::ui::Toc::new("On this <page>")
        .sub("First", "first")
        .item("Delivery", "deli very\"")
        .sub("Abroad <far>", "abroad")
        .sub("Islands", "islands")
        .item("Bad", "!!!")
        .item("Returns", "returns");
    let out = html(&toc, "en");
    assert_eq!(
        out,
        concat!(
            r#"<nav class="z-toc" aria-label="On this &lt;page&gt;"><ol class="z-toc-list">"#,
            r##"<li><a class="z-toc-link" href="#first">First</a></li>"##,
            r##"<li><a class="z-toc-link" href="#delivery">Delivery</a>"##,
            r#"<ol class="z-toc-list z-toc-sub">"#,
            r##"<li><a class="z-toc-link" href="#abroad">Abroad &lt;far&gt;</a></li>"##,
            r##"<li><a class="z-toc-link" href="#islands">Islands</a></li></ol></li>"##,
            r##"<li><a class="z-toc-link" href="#returns">Returns</a></li>"##,
            "</ol></nav>"
        )
    );
    let heading = html(&Heading::h2("Delivery").id("deli very\""), "en");
    assert!(
        heading.contains(r#"id="delivery""#),
        "the same string gives the same anchor on the heading: {heading}"
    );
    let ends_in_sub = html(
        &zeyn::ui::Toc::new("Page")
            .item("A", "a")
            .sub("B", "b"),
        "en",
    );
    assert!(
        ends_in_sub.ends_with("</li></ol></li></ol></nav>"),
        "{ends_in_sub}"
    );
    assert_eq!(
        html(&zeyn::ui::Toc::new("Empty"), "en"),
        r#"<nav class="z-toc" aria-label="Empty"><ol class="z-toc-list"></ol></nav>"#
    );
}

#[test]
fn funnels_size_each_stage_by_its_share_of_the_first() {
    let f = zeyn::ui::Funnel::new("Sales <funnel>")
        .stage("Visits", 12_400)
        .stage("Carts <b>", 3_100)
        .stage("Refunds", 99_999)
        .tone(Tone::Success);
    let out = html(&f, "en");
    assert!(
        out.starts_with(
            r#"<ol class="z-funnel z-tone-success" aria-label="Sales &lt;funnel&gt;">"#
        ),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<span class="z-funnel-name">Carts &lt;b&gt;</span><span class="z-funnel-track"><span class="z-funnel-bar" style="--z-funnel:25%"></span></span><span class="z-funnel-value">3,100</span><span class="z-funnel-share">25%</span>"#
        ),
        "{out}"
    );
    assert!(
        out.contains("--z-funnel:100%") && !out.contains("--z-funnel:806%"),
        "a stage larger than the first is never wider than it: {out}"
    );
    assert!(html(&f, "tr").contains(">12.400<"));
    let empty = html(&zeyn::ui::Funnel::new("None").stage("Zero", 0), "en");
    assert!(empty.contains("--z-funnel:0%"), "{empty}");
    let capped = (0..30).fold(zeyn::ui::Funnel::new("Many"), |f, i| {
        f.stage(i.to_string(), i)
    });
    assert_eq!(
        capped.schema()["stages"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_FUNNEL_STAGES
    );
}

#[test]
fn activity_rings_draw_a_ring_and_a_legend_line_for_each_goal() {
    let rings = zeyn::ui::ActivityRings::new("Today <now>")
        .ring("Packed <b>", 42, 60, Tone::Brand)
        .ring("Calls", 30, 20, Tone::Success)
        .size(9);
    let out = html(&rings, "en");
    assert!(
        out.starts_with(
            r#"<div class="z-rings" role="group" aria-label="Today &lt;now&gt;"><svg class="z-rings-svg" viewBox="0 0 36 36" width="64" height="64" aria-hidden="true" focusable="false">"#
        ),
        "{out}"
    );
    assert!(out.contains(r#"class="z-rings-bar z-tone-brand" cx="18" cy="18" r="16" pathLength="100" stroke-dasharray="70 100""#));
    assert!(
        out.contains(r#"r="12" pathLength="100" stroke-dasharray="100 100""#),
        "a value over its goal is the goal: {out}"
    );
    assert!(out.contains(
        r#"<span class="z-rings-name">Packed &lt;b&gt;</span><span class="z-rings-value">70%</span>"#
    ));
    let capped = (0..9).fold(zeyn::ui::ActivityRings::new("Many"), |r, i| {
        r.ring(i.to_string(), 1, 2, Tone::Info)
    });
    assert_eq!(
        capped.schema()["rings"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_RINGS
    );
}

#[test]
fn comparison_tables_say_included_or_not_and_mark_the_featured_plan() {
    let t = zeyn::ui::ComparisonTable::new("Plans <2026>")
        .plan("Free")
        .plan("Team <b>")
        .featured()
        .row("Users", ["3", "25"])
        .row("Audit log", [false, true])
        .row("Short", [true]);
    let out = html(&t, "en");
    assert!(out.contains(
        r#"<caption class="z-table-caption" id="s-compare-">Plans &lt;2026&gt;</caption>"#
    ));
    assert!(out.contains(
        r#"<thead><tr><td></td><th scope="col">Free</th><th scope="col" class="z-compare-featured">Team &lt;b&gt;</th></tr></thead>"#
    ));
    assert!(
        out.contains(
            r#"<tr><th scope="row">Users</th><td>3</td><td class="z-compare-featured">25</td></tr>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<td><span class="z-compare-no" aria-hidden="true">&#10005;</span><span class="z-sr-only">Not included</span></td><td class="z-compare-featured"><span class="z-compare-yes" aria-hidden="true">&#10003;</span><span class="z-sr-only">Included</span></td>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"<span class="z-sr-only">Included</span></td><td class="z-compare-featured"></td></tr>"#),
        "a short row leaves the rest empty: {out}"
    );
    assert!(html(&t, "tr").contains(r#"<span class="z-sr-only">Yok</span>"#));
    let capped = (0..20).fold(zeyn::ui::ComparisonTable::new("Many"), |t, i| {
        t.plan(i.to_string())
    });
    assert_eq!(
        capped.schema()["plans"]
            .as_array()
            .unwrap()
            .len(),
        zeyn::ui::MAX_COMPARE_PLANS
    );
}

#[test]
fn image_compares_hold_two_allowed_pictures_and_a_hidden_slider() {
    let c = zeyn::ui::ImageCompare::new("Before and <after>")
        .before("/img/before.png", "The old <floor>")
        .after("/img/after.png", "The new floor")
        .size(1200, 800);
    let out = html(&c, "en");
    assert!(
        out.starts_with(r#"<div class="z-imgcompare" data-zeyn-imgcompare style="--z-imgcompare-ratio:1200 / 800;--z-imgcompare-width:1200px">"#),
        "{out}"
    );
    assert!(out.contains(
        r#"<img class="z-imgcompare-img z-imgcompare-before" src="/img/before.png" alt="The old &lt;floor&gt;" width="1200" height="800""#
    ));
    assert!(
        out.find("z-imgcompare-after").unwrap() < out.find("z-imgcompare-before").unwrap(),
        "the first picture lies on top"
    );
    assert!(
        out.contains(
            r#"<input class="z-imgcompare-range" type="range" min="0" max="100" value="50" aria-label="Before and &lt;after&gt;" data-testid="s.imgcompare" hidden>"#
        ),
        "{out}"
    );
    let outside = html(
        &zeyn::ui::ImageCompare::new("Compare")
            .before("https://elsewhere.example/a.png", "A")
            .after("/img/b.png", "B"),
        "en",
    );
    assert!(
        !outside.contains("elsewhere.example") && outside.contains("/img/b.png"),
        "a picture outside img-src is left out: {outside}"
    );
}

#[test]
fn org_charts_are_nested_lists_with_caps_on_boxes_and_depth() {
    use zeyn::ui::{OrgChart, OrgNode};
    let chart = OrgChart::new(
        "Company <org>",
        OrgNode::new("Ada <L>")
            .role("Director")
            .child(OrgNode::new("Alan").role("Engineering <eng>"))
            .child(OrgNode::new("Grace")),
    );
    let out = html(&chart, "en");
    assert_eq!(
        out,
        concat!(
            r#"<div class="z-org" role="group" aria-label="Company &lt;org&gt;" tabindex="0"><ul>"#,
            r#"<li><div class="z-org-box"><span class="z-org-name">Ada &lt;L&gt;</span><span class="z-org-role">Director</span></div><ul>"#,
            r#"<li><div class="z-org-box"><span class="z-org-name">Alan</span><span class="z-org-role">Engineering &lt;eng&gt;</span></div></li>"#,
            r#"<li><div class="z-org-box"><span class="z-org-name">Grace</span></div></li>"#,
            "</ul></li></ul></div>"
        )
    );
    let deep = (0..40).fold(OrgNode::new("leaf"), |child, i| {
        OrgNode::new(i.to_string()).child(child)
    });
    let out = html(&OrgChart::new("Deep", deep), "en");
    assert_eq!(out.matches("z-org-box").count(), zeyn::ui::MAX_ORG_DEPTH);
    let wide = (0..500).fold(OrgNode::new("root"), |root, i| {
        root.child(OrgNode::new(i.to_string()))
    });
    let out = html(&OrgChart::new("Wide", wide), "en");
    assert_eq!(out.matches("z-org-box").count(), zeyn::ui::MAX_ORG_NODES);
}

#[test]
fn gantts_are_tables_with_dates_as_text_and_bars_cut_at_the_span() {
    let day = |d: u8| Date::new(2026, 11, d).unwrap();
    let g = zeyn::ui::Gantt::new("Launch <plan>", day(1), day(30))
        .task("Design <d>", day(1), day(15))
        .done(250)
        .task("Build", day(16), Date::new(2027, 1, 9).unwrap())
        .tone(Tone::Warning)
        .task(
            "Old",
            Date::new(2026, 1, 1).unwrap(),
            Date::new(2026, 2, 1).unwrap(),
        )
        .today(day(16));
    let out = html(&g, "en");
    assert!(out.contains(
        r#"<caption class="z-table-caption" id="s-gantt-">Launch &lt;plan&gt;</caption>"#
    ));
    assert!(out.contains(
        r#"<th scope="col">Task</th><th scope="col">Starts</th><th scope="col">Ends</th><td class="z-gantt-track" aria-hidden="true"><span class="z-gantt-month" style="--z-gantt-at:0.00%">November</span></td>"#
    ), "{out}");
    assert!(
        out.contains(
            r#"<tr><th scope="row">Design &lt;d&gt;<span class="z-sr-only">, 100% done</span></th><td class="z-gantt-date">11/1/2026</td><td class="z-gantt-date">11/15/2026</td><td class="z-gantt-track" aria-hidden="true"><span class="z-gantt-bar z-tone-brand" style="--z-gantt-at:0.00%;--z-gantt-width:50.00%"><span class="z-gantt-done" style="--z-gantt-done:100%"></span></span>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"z-tone-warning" style="--z-gantt-at:50.00%;--z-gantt-width:50.00%""#),
        "a task that ends after the span is cut at its end: {out}"
    );
    assert_eq!(
        out.matches("z-gantt-bar").count(),
        2,
        "a task outside the span has no bar but keeps its row: {out}"
    );
    assert!(out.contains(r#"<th scope="row">Old</th><td class="z-gantt-date">1/1/2026</td>"#));
    assert_eq!(
        out.matches(r#"<span class="z-gantt-today" style="--z-gantt-at:50.00%"></span>"#)
            .count(),
        3
    );
    let tr = html(&g, "tr");
    assert!(tr.contains(">Kasım<") && tr.contains(r#"<th scope="col">Görev</th>"#));

    let long = zeyn::ui::Gantt::new("Long", day(1), Date::new(2040, 1, 1).unwrap());
    assert_eq!(long.schema()["end"], "2028-10-31", "the span is capped");
    let back = zeyn::ui::Gantt::new("Back", day(9), day(2));
    assert_eq!(back.schema()["end"], "2026-11-09");
}

#[test]
fn tours_render_a_hidden_button_and_stops_keep_their_component() {
    use zeyn::ui::{Tour, TourStop};
    let out = html(&Tour::new("Show <me> around"), "en");
    assert!(
        out.starts_with(
            r#"<div class="z-tour" data-zeyn-tour><button type="button" class="z-button z-tour-start" data-testid="s.tour" hidden>Show &lt;me&gt; around</button><dialog class="z-tour-card""#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"data-progress="Step {n} of {total}""#),
        "{out}"
    );
    assert!(
        out.contains(r#"data-next="Next" data-finish="Finish""#),
        "{out}"
    );
    let tr = html(&Tour::new("Tur"), "tr");
    assert!(
        tr.contains(r#"data-finish="Bitir""#) && tr.contains(">Kapat</button>"),
        "{tr}"
    );

    let stop = TourStop::on(Badge::new("Draft"))
        .title("Status <s>")
        .body("A \"draft\" is not sent.");
    let out = html(&stop, "en");
    assert!(
        out.starts_with(
            r#"<div class="z-tourstop" data-tour-title="Status &lt;s&gt;" data-tour-body="A &quot;draft&quot; is not sent.">"#
        ),
        "{out}"
    );
    assert!(out.contains("Draft") && out.ends_with("</div>"));
    assert_eq!(stop.children().len(), 1);
}

#[test]
fn flow_diagrams_place_steps_in_columns_and_name_what_each_leads_to() {
    use zeyn::ui::FlowDiagram;
    let flow = FlowDiagram::new("Order <process>")
        .node("cart", "Cart")
        .node("pay", "Payment <p>")
        .node("ship", "Shipping")
        .tone(Tone::Success)
        .node("cancel", "Cancelled")
        .node("cart", "Twice")
        .edge("cart", "pay")
        .edge_with("pay", "ship", "Paid <ok>")
        .edge("pay", "cancel")
        .edge("ship", "cart")
        .edge("pay", "nowhere")
        .edge("pay", "pay");
    let out = html(&flow, "en");
    assert!(
        out.starts_with(
            r#"<div class="z-flow" role="group" aria-label="Order &lt;process&gt;" tabindex="0"><div class="z-flow-canvas" style="--z-flow-w:648px;--z-flow-h:192px">"#
        ),
        "three columns, two rows and room for the arrow back: {out}"
    );
    assert_eq!(
        out.matches("<li class=").count(),
        4,
        "a repeated id is left out: {out}"
    );
    assert_eq!(
        out.matches(r#"<path class="z-flow-line""#)
            .count(),
        4,
        "arrows to an unknown step or to the step itself are left out: {out}"
    );
    assert!(
        out.contains(r#"<li class="z-flow-node" style="--z-flow-x:0px;--z-flow-y:40px"><span class="z-flow-title">Cart</span><span class="z-sr-only">Next: Payment &lt;p&gt;</span></li>"#),
        "{out}"
    );
    assert!(
        out.contains(r#"<li class="z-flow-node z-tone-success" style="--z-flow-x:480px;--z-flow-y:0px"><span class="z-flow-title">Shipping</span><span class="z-sr-only">Next: Cart</span></li>"#),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<span class="z-sr-only">Next: Shipping (Paid &lt;ok&gt;), Cancelled</span>"#
        ),
        "{out}"
    );
    assert!(
        out.contains(r#"aria-hidden="true">Paid &lt;ok&gt;</span>"#),
        "{out}"
    );
    assert!(html(&flow, "tr").contains("Sonraki: Cart"));

    let mut many = FlowDiagram::new("Many");
    for i in 0..60 {
        many = many.node(format!("n{i}"), "Step");
    }
    let out = html(&many, "en");
    assert_eq!(out.matches("<li class=").count(), zeyn::ui::MAX_FLOW_NODES);
    assert!(html(&FlowDiagram::new("Empty"), "en").contains("--z-flow-w:0px;--z-flow-h:0px"));
}

#[test]
fn org_chart_schemas_keep_the_limits_of_the_drawing() {
    let mut root = OrgNode::new("Root");
    for i in 0..300 {
        root = root.child(OrgNode::new(format!("Person {i}")));
    }
    let chart = OrgChart::new("Company", root);
    let schema = chart.schema();
    let boxes = schema["root"]["children"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(boxes + 1, zeyn::ui::MAX_ORG_NODES);
}
