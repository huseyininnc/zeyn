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
