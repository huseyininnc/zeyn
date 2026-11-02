//! Date and time pickers: natural input read on the server, locale formats, options, ranges,
//! daylight saving gaps and the check endpoint.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, DateWords, I18n, Identity, Render, range_presets};

#[model]
pub struct Booking {
    #[rule(required, min = 2024_01_01, max = 2024_12_31)]
    pub day: Option<Date>,
    pub at: Time,
    pub slot: LocalDateTime,
    pub moment: DateTime,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::new(y, m, day).unwrap()
}

fn start() -> Booking {
    Booking {
        slot: LocalDateTime::new(d(2024, 3, 1), Time::default()),
        moment: DateTime::from_unix_ms(1_700_000_000_000),
        ..Booking::default()
    }
}

fn form() -> Form<Booking> {
    Form::of(start())
        .field(Booking::day)
        .field(Booking::at)
        .field(Booking::slot)
        .field(Booking::moment)
        .range(Booking::from, Booking::to)
}

fn tr() -> DateWords {
    DateWords::from_i18n(I18n::builtin(), "tr")
}

const TODAY: (i32, u8, u8) = (2024, 3, 15);

fn date(s: &str, locale: &str) -> Option<Date> {
    let words = if locale.starts_with("tr") { tr() } else { DateWords::english() };
    Date::parse_input(s, locale, d(TODAY.0, TODAY.1, TODAY.2), &words)
}

#[test]
fn partial_dates_follow_the_locale_order_and_fill_in_this_year() {
    assert_eq!(date("5", "tr"), Some(d(2024, 3, 5)));
    assert_eq!(date("5.1", "tr"), Some(d(2024, 1, 5)));
    assert_eq!(date("05.01.24", "tr"), Some(d(2024, 1, 5)));
    assert_eq!(date("05012024", "tr"), Some(d(2024, 1, 5)));
    assert_eq!(date("0501", "tr"), Some(d(2024, 1, 5)));
    assert_eq!(date("05/01/2024", "en-GB"), Some(d(2024, 1, 5)));
    assert_eq!(date("1/5", "en-US"), Some(d(2024, 1, 5)));
    assert_eq!(date("1/5/24", "en-US"), Some(d(2024, 1, 5)));
    assert_eq!(date("01052024", "en-US"), Some(d(2024, 1, 5)));
    assert_eq!(date("2024-01-05", "tr"), Some(d(2024, 1, 5)));
    assert_eq!(date("01-05", "ja"), Some(d(2024, 1, 5)));
}

#[test]
fn words_and_relative_input_are_read_in_the_users_language() {
    assert_eq!(date("bugün", "tr"), Some(d(2024, 3, 15)));
    assert_eq!(date("BUGÜN", "tr"), Some(d(2024, 3, 15)));
    assert_eq!(date("yarın", "tr"), Some(d(2024, 3, 16)));
    assert_eq!(date("dün", "tr"), Some(d(2024, 3, 14)));
    assert_eq!(date("today", "tr"), Some(d(2024, 3, 15)), "English words always work");
    assert_eq!(date("+3g", "tr"), Some(d(2024, 3, 18)));
    assert_eq!(date("-1h", "tr"), Some(d(2024, 3, 8)), "h is a week (hafta) in Turkish");
    assert_eq!(date("+2a", "tr"), Some(d(2024, 5, 15)));
    assert_eq!(date("+1y", "tr"), Some(d(2025, 3, 15)));
    assert_eq!(date("+3", "en"), Some(d(2024, 3, 18)), "a bare number is days");
    assert_eq!(date("- 2 weeks", "en"), Some(d(2024, 3, 1)));
    assert_eq!(date("+1m", "en"), Some(d(2024, 4, 15)));
    let jan31 = d(2024, 1, 31);
    assert_eq!(
        Date::parse_input("+1m", "en", jan31, &DateWords::english()),
        Some(d(2024, 2, 29)),
        "month ends are kept"
    );
}

#[test]
fn impossible_dates_are_refused() {
    for bad in ["31.02", "32", "5.13", "abc", "+99999g", "+3x", "1.2.3.4", "05.01.2O24", "", "123"] {
        assert_eq!(date(bad, "tr"), None, "{bad:?}");
    }
}

#[test]
fn times_accept_digits_words_and_both_clocks() {
    let now = Time::new(10, 7, 0).unwrap();
    let t = |s| Time::parse_input(s, "tr", now, &tr());
    assert_eq!(t("930"), Time::new(9, 30, 0));
    assert_eq!(t("1430"), Time::new(14, 30, 0));
    assert_eq!(t("14"), Time::new(14, 0, 0));
    assert_eq!(t("14.30"), Time::new(14, 30, 0));
    assert_eq!(t("2pm"), Time::new(14, 0, 0));
    assert_eq!(t("2:30 PM"), Time::new(14, 30, 0));
    assert_eq!(t("şimdi"), Some(now));
    for bad in ["2460", "13pm", "0am", "25", "x", "14:3"] {
        assert_eq!(t(bad), None, "{bad:?}");
    }
    assert_eq!(Time::parse_local("12:15 am", "en"), Time::new(0, 15, 0));
}

#[test]
fn date_times_combine_a_date_and_a_time() {
    let now = LocalDateTime::new(d(2024, 3, 15), Time::new(10, 7, 0).unwrap());
    let at = |day: Date, h, m| Some(LocalDateTime::new(day, Time::new(h, m, 0).unwrap()));
    let l = |s| LocalDateTime::parse_input(s, "tr", now, &tr());
    assert_eq!(l("yarın 9"), at(d(2024, 3, 16), 9, 0));
    assert_eq!(l("+1g 14:30"), at(d(2024, 3, 16), 14, 30));
    assert_eq!(l("5.1 930"), at(d(2024, 1, 5), 9, 30));
    assert_eq!(l("bugün"), at(d(2024, 3, 15), 0, 0));
    assert_eq!(l("şimdi"), Some(now));
    assert_eq!(l("05.01.2024 14:30"), at(d(2024, 1, 5), 14, 30));
    assert_eq!(
        LocalDateTime::parse_input("1/5/2024 2:30 PM", "en-US", now, &DateWords::english()),
        at(d(2024, 1, 5), 14, 30)
    );
    assert_eq!(l("yarın 25:00"), None);
}

#[test]
fn quick_ranges_are_worked_out_from_today() {
    let presets = range_presets(d(2024, 3, 15));
    let get = |k: &str| presets.iter().find(|p| p.0 == k).map(|p| (p.1, p.2)).unwrap();
    assert_eq!(get("today"), (d(2024, 3, 15), d(2024, 3, 15)));
    assert_eq!(get("yesterday"), (d(2024, 3, 14), d(2024, 3, 14)));
    assert_eq!(get("last_7_days"), (d(2024, 3, 9), d(2024, 3, 15)));
    assert_eq!(get("this_month"), (d(2024, 3, 1), d(2024, 3, 31)));
    assert_eq!(get("last_month"), (d(2024, 2, 1), d(2024, 2, 29)));
    assert_eq!(get("this_year"), (d(2024, 1, 1), d(2024, 12, 31)));
    let january = range_presets(d(2024, 1, 10));
    assert!(january.iter().any(|p| p.0 == "last_month" && p.1 == d(2023, 12, 1) && p.2 == d(2023, 12, 31)));
}

fn values(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), serde_json::json!(v))).collect()
}

const BASE: [(&str, &str); 4] =
    [("day", "05.01.2024"), ("at", "09:00"), ("slot", "05.01.2024 09:00"), ("moment", "05.01.2024 14:30")];

#[test]
fn the_server_reads_typed_values_and_reports_daylight_saving_gaps() {
    let berlin = TimeZone::named("Europe/Berlin").unwrap();
    let ok = form().read(&values(&BASE), "tr", berlin).unwrap();
    assert_eq!(ok.day, Some(d(2024, 1, 5)));
    assert_eq!(ok.moment.to_string(), "2024-01-05T13:30:00.000Z");

    let mut gap = BASE.to_vec();
    gap[3] = ("moment", "31.03.2024 02:30");
    let errors = form().read(&values(&gap), "tr", berlin).unwrap_err();
    assert_eq!(errors.errors.len(), 1);
    assert_eq!(errors.errors[0].field, "moment");
    assert!(
        errors.errors[0].message.english().contains("does not exist in your time zone"),
        "{}",
        errors.errors[0].message.english()
    );

    let mut late = BASE.to_vec();
    late[0] = ("day", "05.01.2025");
    let errors = form().read(&values(&late), "tr", berlin).unwrap_err();
    assert!(errors.errors[0].message.english().contains("2024-12-31"));
}

#[test]
fn picker_options_and_range_order_are_checked_on_the_server() {
    let tz = TimeZone::UTC;
    let options = form()
        .date_options(Booking::day, DateOptions::new().no_weekends().disable([d(2024, 1, 2)]))
        .date_options(Booking::at, DateOptions::new().step_minutes(15));
    let field_errors = |pairs: &[(&str, &str)]| -> Vec<(&'static str, String)> {
        match options.read(&values(pairs), "en", tz) {
            Ok(_) => Vec::new(),
            Err(e) => e.errors.iter().map(|x| (x.field, x.message.english())).collect(),
        }
    };
    let mut v = BASE.to_vec();
    v[0] = ("day", "2024-01-06");
    v[1] = ("at", "09:10");
    v.extend([("from", "2024-02-10"), ("to", "2024-02-01")]);
    let errors = field_errors(&v);
    assert_eq!(errors.len(), 3, "{errors:?}");
    assert_eq!(errors[0], ("day", "Day: this day cannot be chosen.".to_string()));
    assert_eq!(errors[1], ("at", "At must be on a 15 minute step.".to_string()));
    assert_eq!(errors[2], ("to", "To cannot be before From.".to_string()));

    v[0] = ("day", "2024-01-02");
    v[1] = ("at", "09:15");
    v[4] = ("from", "2024-02-05");
    assert_eq!(field_errors(&v).iter().map(|e| e.0).collect::<Vec<_>>(), ["day", "to"]);
    v[0] = ("day", "2024-01-03");
    v[4] = ("from", "2024-02-01");
    assert!(field_errors(&v).is_empty());
}

fn html(locale: &str, booking: Booking, tz: TimeZone) -> String {
    let mut out = String::new();
    let f = Form::of(booking)
        .field(Booking::day)
        .field(Booking::at)
        .field(Booking::slot)
        .field(Booking::moment)
        .range(Booking::from, Booking::to)
        .date_options(Booking::day, DateOptions::new().no_weekends().week_numbers())
        .date_options(Booking::at, DateOptions::new().step_minutes(5));
    f.render(&mut out, &Render::new("book", locale, I18n::builtin()).with_tz(tz));
    out
}

#[test]
fn editors_are_custom_text_inputs_in_the_page_language() {
    let istanbul = TimeZone::named("Europe/Istanbul").unwrap();
    let b = Booking {
        day: Some(d(2024, 1, 5)),
        at: Time::new(14, 30, 0).unwrap(),
        slot: LocalDateTime::new(d(2024, 1, 5), Time::new(9, 0, 0).unwrap()),
        moment: DateTime::parse("2024-01-05T11:30:00Z").unwrap(),
        from: Some(d(2024, 1, 1)),
        to: Some(d(2024, 1, 31)),
    };
    let tr = html("tr", b.clone(), istanbul);
    for native in [r#"type="date""#, r#"type="time""#, r#"type="datetime-local""#] {
        assert!(!tr.contains(native), "no native {native}");
    }
    assert!(tr.contains(r#"value="05.01.2024""#));
    assert!(tr.contains(r#"value="05.01.2024 09:00""#));
    assert!(tr.contains(r#"value="05.01.2024 14:30""#), "moments are shown in the user's zone");
    assert!(tr.contains(r#"data-order="dmy" data-sep="." data-pad="true" data-h12="false" data-first-day="1""#));
    assert!(tr.contains(r#"data-min="2024-01-01" data-max="2024-12-31""#));
    assert!(tr.contains(r#"data-weekends="off" data-weeks="true""#));
    assert!(tr.contains(r#"data-step="5""#));
    assert!(tr.contains("Biçim: 31.12.2024. Şunları da yazabilirsiniz: bugün, +3g."));
    assert!(tr.contains(r#"aria-describedby="book-moment-hint book-moment-zone book-moment-error""#));
    assert!(tr.contains(r#"title="UTC: 2024-01-05T11:30:00.000Z""#));
    assert!(tr.contains("Europe/Istanbul"));
    assert!(tr.contains(r#"data-zeyn-date="range""#) && tr.contains("&quot;key&quot;:&quot;last_7_days&quot;"));
    assert!(tr.contains(r#"value="01.01.2024""#) && tr.contains(r#"value="31.01.2024""#));
    assert!(tr.contains(r#"id="book-to-error""#));

    let en = html("en", b, TimeZone::UTC);
    assert!(en.contains(r#"value="1/5/2024""#));
    assert!(en.contains(r#"value="2:30 PM""#));
    assert!(en.contains(r#"data-h12="true" data-first-day="7""#));
    assert!(en.contains("Format: 12/31/2024. You can also type: today, +3d."));
    assert_eq!(en.matches("data-zeyn-date-open").count(), 5, "every editor has a picker button");
}

#[test]
fn values_that_need_markup_are_escaped() {
    let out = html("en\"><script>", start(), TimeZone::UTC);
    assert!(!out.contains("<script>"));
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en").catalog("tr", "{}").unwrap())
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(|| Identity::user("u").with_time_zone("Europe/Berlin")))
        .fixed_now(DateTime::parse("2024-03-15T10:00:00Z").unwrap())
        .feature(Feature::new("none"))
        .router()
        .unwrap()
}

async fn parse_call(body: serde_json::Value, csrf: bool, locale: &str) -> (StatusCode, serde_json::Value) {
    let mut req = Request::post("/_zeyn/parse")
        .header("x-user", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}; zeyn_locale={locale}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = app().oneshot(req.body(Body::from(body.to_string())).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn the_check_endpoint_answers_canonical_text_in_the_users_language_and_zone() {
    let (status, r) = parse_call(serde_json::json!({ "kind": "date", "text": "yarın" }), true, "tr").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["text"], "16.03.2024");
    assert_eq!(r["iso"], "2024-03-16");

    let (_, r) = parse_call(serde_json::json!({ "kind": "datetime", "text": "15.03.2024 14:30" }), true, "tr").await;
    assert_eq!(r["utc"], "2024-03-15T13:30:00.000Z");
    assert_eq!(r["zone"], "Europe/Berlin");

    let (_, r) = parse_call(
        serde_json::json!({ "kind": "datetime", "text": "31.03.2024 02:30", "label": "Onay <b>" }),
        true,
        "tr",
    )
    .await;
    assert_eq!(r["ok"], false);
    assert!(r["message"].as_str().unwrap().starts_with("Onay <b>: bu saat"), "{r}");

    let (_, r) = parse_call(serde_json::json!({ "kind": "time", "text": "now" }), true, "en").await;
    assert_eq!(r["text"], "11:00 AM", "now in the user's zone");

    let (_, r) = parse_call(serde_json::json!({ "kind": "presets" }), true, "en").await;
    assert_eq!(r["today"], "2024-03-15");
    assert_eq!(r["data"]["order"], "mdy");
    assert_eq!(r["presets"][2]["from"], "2024-03-09");

    let (status, _) = parse_call(serde_json::json!({ "kind": "date", "text": "today" }), false, "en").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "posts need the CSRF token");
    let (status, _) = parse_call(serde_json::json!({ "kind": "nope", "text": "x" }), true, "en").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
