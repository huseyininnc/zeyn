use zeyn::DateWords;
use zeyn::prelude::*;
use zeyn::render::Render;
use zeyn::{Component, Ctx, Identity, Services, Value};

fn tz(name: &str) -> TimeZone {
    TimeZone::named(name).unwrap()
}

fn at(s: &str) -> DateTime {
    DateTime::parse(s).unwrap()
}

fn local(s: &str) -> LocalDateTime {
    LocalDateTime::parse(s).unwrap()
}

#[test]
fn dates_round_trip_through_day_numbers() {
    for days in [-719_162, -1, 0, 1, 11_016, 19_723, 2_932_896] {
        let d = Date::from_days_since_epoch(days).unwrap();
        assert_eq!(d.days_since_epoch(), days);
    }
    assert_eq!(
        Date::new(1970, 1, 1)
            .unwrap()
            .days_since_epoch(),
        0
    );
    assert_eq!(
        Date::new(2024, 2, 29)
            .unwrap()
            .weekday(),
        4
    );
    assert!(Date::new(2023, 2, 29).is_none());
    assert!(Date::new(2100, 2, 29).is_none());
    assert!(Date::new(2000, 2, 29).is_some());
    assert_eq!(
        Date::new(2024, 12, 31)
            .unwrap()
            .add_days(1),
        Date::new(2025, 1, 1)
    );
}

#[test]
fn dates_are_read_and_written_per_locale() {
    let d = Date::new(2024, 1, 5).unwrap();
    assert_eq!(d.to_string(), "2024-01-05");
    assert_eq!(d.format("tr"), "05.01.2024");
    assert_eq!(d.format("en-US"), "1/5/2024");
    assert_eq!(d.format("en-GB"), "05/01/2024");
    assert_eq!(Date::parse_local("05.01.2024", "tr"), Some(d));
    assert_eq!(Date::parse_local("1/5/2024", "en"), Some(d));
    assert_eq!(Date::parse_local("2024-01-05", "tr"), Some(d));
    assert_eq!(Date::parse_local("31.02.2024", "tr"), None);
    assert_eq!(
        Time::new(14, 30, 0)
            .unwrap()
            .format("en-US"),
        "2:30 PM"
    );
    assert_eq!(
        Time::new(0, 5, 0)
            .unwrap()
            .format("en-US"),
        "12:05 AM"
    );
    assert_eq!(
        Time::new(14, 30, 0)
            .unwrap()
            .format("tr"),
        "14:30"
    );
}

#[test]
fn a_date_never_shifts_with_the_time_zone() {
    let birth: Date = serde_json::from_str("\"1990-03-15\"").unwrap();
    assert_eq!(serde_json::to_string(&birth).unwrap(), "\"1990-03-15\"");
    let mut out = String::new();
    let r = Render::english("x").with_tz(tz("America/Los_Angeles"));
    out.push_str(&r.value(&Value::Date(birth), &zeyn::FieldKind::Date));
    assert!(out.contains("3/15/1990"), "{out}");
}

#[test]
fn moments_parse_with_offsets_and_print_in_utc() {
    assert_eq!(at("2024-01-05T14:30:00+03:00"), at("2024-01-05T11:30:00Z"));
    assert_eq!(at("2024-01-05T11:30:00.5Z").unix_ms() % 1000, 500);
    assert_eq!(
        at("2024-01-05T11:30:00Z").to_string(),
        "2024-01-05T11:30:00.000Z"
    );
    assert!(DateTime::parse("2024-01-05 11:30").is_none());
    let back: DateTime = serde_json::from_str("\"2024-01-05T11:30:00Z\"").unwrap();
    assert_eq!(back, at("2024-01-05T11:30:00Z"));
}

#[test]
fn istanbul_is_utc_plus_three_all_year() {
    let ist = tz("Europe/Istanbul");
    assert_eq!(
        at("2024-01-05T11:30:00Z").local(ist),
        local("2024-01-05T14:30:00")
    );
    assert_eq!(
        at("2024-07-05T11:30:00Z").local(ist),
        local("2024-07-05T14:30:00")
    );
    assert_eq!(
        at("2024-01-05T22:30:00Z")
            .local(ist)
            .date,
        Date::new(2024, 1, 6).unwrap()
    );
}

#[test]
fn european_daylight_saving_switches_at_one_utc() {
    let berlin = tz("Europe/Berlin");
    assert_eq!(berlin.offset_minutes_at(at("2024-03-31T00:59:59Z")), 60);
    assert_eq!(berlin.offset_minutes_at(at("2024-03-31T01:00:00Z")), 120);
    assert_eq!(berlin.offset_minutes_at(at("2024-10-27T00:59:59Z")), 120);
    assert_eq!(berlin.offset_minutes_at(at("2024-10-27T01:00:00Z")), 60);
    assert_eq!(
        tz("Europe/London").offset_minutes_at(at("2024-06-01T12:00:00Z")),
        60
    );
}

#[test]
fn us_daylight_saving_switches_at_two_local() {
    let ny = tz("America/New_York");
    assert_eq!(ny.offset_minutes_at(at("2024-03-10T06:59:59Z")), -300);
    assert_eq!(ny.offset_minutes_at(at("2024-03-10T07:00:00Z")), -240);
    assert_eq!(ny.offset_minutes_at(at("2024-11-03T05:59:59Z")), -240);
    assert_eq!(ny.offset_minutes_at(at("2024-11-03T06:00:00Z")), -300);
}

#[test]
fn skipped_and_repeated_wall_times_are_handled() {
    let berlin = tz("Europe/Berlin");
    assert_eq!(
        DateTime::from_local(local("2024-03-31T02:30:00"), berlin),
        Err(zeyn::LocalTimeError::Skipped)
    );
    assert_eq!(
        DateTime::from_local(local("2024-10-27T02:30:00"), berlin),
        Ok(at("2024-10-27T00:30:00Z"))
    );
    assert_eq!(
        DateTime::from_local(local("2024-06-01T10:00:00"), berlin),
        Ok(at("2024-06-01T08:00:00Z"))
    );
    let ny = tz("America/New_York");
    assert_eq!(
        DateTime::from_local(local("2024-03-10T02:15:00"), ny),
        Err(zeyn::LocalTimeError::Skipped)
    );
    assert_eq!(
        DateTime::from_local(local("2024-11-03T01:30:00"), ny),
        Ok(at("2024-11-03T05:30:00Z"))
    );
}

#[test]
fn istanbul_history_follows_the_2016_move_to_permanent_plus_three() {
    let ist = tz("Europe/Istanbul");
    assert_eq!(ist.offset_minutes_at(at("2015-07-01T12:00:00Z")), 180);
    assert_eq!(ist.offset_minutes_at(at("2015-12-01T12:00:00Z")), 120);
    assert_eq!(ist.offset_minutes_at(at("2016-12-01T12:00:00Z")), 180);
    assert_eq!(
        at("2015-12-01T12:00:00Z").local(ist),
        local("2015-12-01T14:00:00")
    );
    assert_eq!(
        DateTime::from_local(local("2015-03-29T03:30:00"), ist),
        Err(zeyn::LocalTimeError::Skipped)
    );
    assert_eq!(
        DateTime::from_local(local("2015-11-08T03:30:00"), ist),
        Ok(at("2015-11-08T00:30:00Z"))
    );
}

#[test]
fn old_us_rules_start_in_april_and_end_in_october() {
    let ny = tz("America/New_York");
    assert_eq!(ny.offset_minutes_at(at("2006-04-02T06:59:59Z")), -300);
    assert_eq!(ny.offset_minutes_at(at("2006-04-02T07:00:00Z")), -240);
    assert_eq!(ny.offset_minutes_at(at("2006-10-29T05:59:59Z")), -240);
    assert_eq!(ny.offset_minutes_at(at("2006-10-29T06:00:00Z")), -300);
    assert_eq!(ny.offset_minutes_at(at("2006-03-12T12:00:00Z")), -300);
}

#[test]
fn every_iana_zone_is_known_in_any_case() {
    assert_eq!(tz("europe/istanbul").name(), "Europe/Istanbul");
    assert_eq!(tz("europe/istanbul"), tz("Europe/Istanbul"));
    assert_eq!(
        tz("Asia/Kathmandu").offset_minutes_at(at("2024-01-01T00:00:00Z")),
        345
    );
    let sydney = tz("Australia/Sydney");
    assert_eq!(sydney.offset_minutes_at(at("2024-01-15T00:00:00Z")), 660);
    assert_eq!(sydney.offset_minutes_at(at("2024-07-15T00:00:00Z")), 600);
    assert!(TimeZone::names().all(|n| TimeZone::named(n).is_some()));
    assert!(TimeZone::named("Europe/Atlantis").is_none());
    assert!(TimeZone::named(&"x".repeat(5000)).is_none());
}

#[test]
fn fixed_offsets_and_unknown_zones() {
    assert_eq!(tz("UTC+05:30").offset_minutes_at(DateTime::now()), 330);
    assert_eq!(tz("+03:00").name(), "UTC+03:00");
    assert!(TimeZone::named("Mars/Olympus").is_none());
    assert!(TimeZone::named("UTC+15:00").is_none());
}

#[test]
fn today_depends_on_the_users_time_zone() {
    let now = at("2024-01-05T22:30:00Z");
    let ctx = Ctx::for_tests(Identity::anonymous(), Services::default()).with_now(now);
    assert_eq!(ctx.today(), Date::new(2024, 1, 5).unwrap());
    assert_eq!(
        ctx.with_time_zone(tz("Europe/Istanbul"))
            .today(),
        Date::new(2024, 1, 6).unwrap()
    );
}

/// An appointment: a calendar day, a wall clock time and the moment it was booked.
#[model]
pub struct Appointment {
    pub day: Date,
    pub starts: Time,
    pub booked_at: DateTime,
    pub at_clinic: LocalDateTime,
    pub follow_up: Option<Date>,
}

#[action(public)]
pub async fn book(a: Valid<Appointment>) -> Result<Appointment> {
    Ok(a.into_inner())
}

fn form() -> zeyn::SubmitForm<Appointment, book> {
    let a = Appointment {
        day: Date::new(2024, 1, 5).unwrap(),
        starts: Time::new(9, 0, 0).unwrap(),
        booked_at: at("2024-01-05T06:00:00Z"),
        at_clinic: local("2024-01-05T09:00:00"),
        follow_up: None,
    };
    Form::of(a)
        .field(Appointment::day)
        .field(Appointment::starts)
        .field(Appointment::booked_at)
        .field(Appointment::at_clinic)
        .field(Appointment::follow_up)
        .submit("Book", book)
}

#[test]
fn form_inputs_show_moments_in_the_users_zone_and_dates_as_they_are() {
    let mut out = String::new();
    let r = Render::english("appt").with_tz(tz("Europe/Istanbul"));
    form().render(&mut out, &r);
    assert!(out.contains(r#"id="appt-day""#) && out.contains(r#"value="1/5/2024""#));
    assert!(out.contains(r#"id="appt-starts""#) && out.contains(r#"value="9:00 AM""#));
    assert!(out.contains(r#"id="appt-booked_at""#) && out.contains(r#"value="1/5/2024 9:00 AM""#));
    assert!(out.contains(r#"id="appt-at_clinic""#));
}

#[test]
fn form_reads_wall_clock_input_in_the_users_zone() {
    let values = serde_json::json!({
        "day": "06.01.2024",
        "starts": "10:15",
        "booked_at": "2024-01-05T12:00",
        "at_clinic": "2024-01-06T10:15",
        "follow_up": ""
    });
    let valid = form()
        .form()
        .read(values.as_object().unwrap(), "tr", tz("Europe/Istanbul"))
        .unwrap();
    assert_eq!(valid.day, Date::new(2024, 1, 6).unwrap());
    assert_eq!(valid.starts, Time::new(10, 15, 0).unwrap());
    assert_eq!(valid.booked_at, at("2024-01-05T09:00:00Z"));
    assert_eq!(valid.at_clinic, local("2024-01-06T10:15:00"));
    assert_eq!(valid.follow_up, None);

    let exact = serde_json::json!({ "booked_at": "2024-01-05T12:00:00Z" });
    let valid = form()
        .form()
        .read(exact.as_object().unwrap(), "tr", tz("Europe/Istanbul"))
        .unwrap();
    assert_eq!(valid.booked_at, at("2024-01-05T12:00:00Z"));
}

#[test]
fn form_rejects_times_skipped_by_daylight_saving() {
    let values = serde_json::json!({ "booked_at": "2024-03-31T02:30", "day": "bad" });
    let errors = form()
        .form()
        .read(values.as_object().unwrap(), "en", tz("Europe/Berlin"))
        .unwrap_err();
    let messages: Vec<String> = errors
        .errors
        .iter()
        .map(|e| e.message.english())
        .collect();
    assert!(
        messages.contains(&"Day has an invalid value.".to_string()),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("does not exist in your time zone")),
        "{messages:?}"
    );
}

#[test]
fn grid_sorts_dates_chronologically() {
    let rows = vec![
        Appointment {
            day: Date::new(2024, 3, 1).unwrap(),
            ..Default::default()
        },
        Appointment {
            day: Date::new(2023, 12, 31).unwrap(),
            ..Default::default()
        },
        Appointment {
            day: Date::new(2024, 1, 15).unwrap(),
            ..Default::default()
        },
    ];
    let q: DataQuery = serde_json::from_str(r#"{"sort":[{"field":"day","desc":false}]}"#).unwrap();
    let days: Vec<String> = q
        .apply(rows)
        .items
        .iter()
        .map(|a| a.day.to_string())
        .collect();
    assert_eq!(days, ["2023-12-31", "2024-01-15", "2024-03-01"]);
}

/// Catalogs of the framework's own locale files, loaded from disk so the test covers every
/// language whether or not its `lang-*` feature is on.
fn locale_files() -> Vec<(String, serde_json::Value)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut files: Vec<(String, serde_json::Value)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x == "json")
        })
        .map(|p| {
            let locale = p
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let json = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (locale, json)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn words_of(json: &serde_json::Value, key: &str) -> Vec<String> {
    json["zeyn"]["date"]["words"][key]
        .as_str()
        .unwrap()
        .split(',')
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

fn words_for(locale: &str) -> DateWords {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("locales/{locale}.json"));
    let json = std::fs::read_to_string(path).unwrap();
    let i18n = zeyn::I18n::new("en")
        .catalog(locale, &json)
        .unwrap();
    DateWords::from_i18n(&i18n, locale)
}

#[test]
fn no_date_word_means_two_things_in_one_language() {
    const KEYS: [&str; 8] = [
        "today",
        "tomorrow",
        "yesterday",
        "now",
        "day",
        "week",
        "month",
        "year",
    ];
    let files = locale_files();
    assert!(files.len() >= 9, "every language has a locale file");
    for (locale, json) in &files {
        let mut seen: Vec<(String, &str)> = Vec::new();
        for key in KEYS {
            for w in words_of(json, key) {
                if let Some((_, other)) = seen
                    .iter()
                    .find(|(s, k)| *s == w && *k != key)
                {
                    panic!("`{w}` is both `{other}` and `{key}` in locales/{locale}.json");
                }
                seen.push((w, key));
            }
        }
    }
}

#[test]
fn english_date_words_work_in_every_language_unless_the_language_uses_the_word() {
    let today = Date::new(2024, 3, 15).unwrap();
    let english = [
        ("+3d", 3),
        ("+3day", 3),
        ("+1w", 7),
        ("+1week", 7),
        ("today", 0),
        ("tomorrow", 1),
        ("yesterday", -1),
    ];
    for (locale, json) in locale_files() {
        let words = words_for(&locale);
        let own: Vec<String> = [
            "day",
            "week",
            "month",
            "year",
            "today",
            "tomorrow",
            "yesterday",
        ]
        .iter()
        .flat_map(|k| words_of(&json, k))
        .collect();
        for (input, days) in english {
            let word = input.trim_start_matches(['+', '1', '3']);
            if own.iter().any(|w| w == word) && locale != "en" {
                continue;
            }
            assert_eq!(
                Date::parse_input(input, &locale, today, &words),
                today.add_days(days),
                "{input} in {locale}"
            );
        }
        assert_eq!(
            Date::parse_input("+2m", &locale, today, &words).is_some(),
            true,
            "+2m in {locale}"
        );
        assert_eq!(
            Date::parse_input("+1y", &locale, today, &words).is_some(),
            true,
            "+1y in {locale}"
        );
    }
}

#[test]
fn translated_date_words_are_read() {
    let today = Date::new(2024, 3, 15).unwrap();
    let cases = [
        ("tr", "yarın", Date::new(2024, 3, 16)),
        ("tr", "+2g", Date::new(2024, 3, 17)),
        ("tr", "+1h", Date::new(2024, 3, 22)),
        ("tr", "+1a", Date::new(2024, 4, 15)),
        ("ru", "Завтра", Date::new(2024, 3, 16)),
        ("ru", "+3 дня", Date::new(2024, 3, 18)),
        ("ru", "-1н", Date::new(2024, 3, 8)),
        ("ru", "+1м", Date::new(2024, 4, 15)),
        ("ru", "+1 год", Date::new(2025, 3, 15)),
        ("zh", "明天", Date::new(2024, 3, 16)),
        ("zh", "+3天", Date::new(2024, 3, 18)),
        ("zh", "+1周", Date::new(2024, 3, 22)),
        ("zh", "+1月", Date::new(2024, 4, 15)),
        ("es", "mañana", Date::new(2024, 3, 16)),
        ("es", "AYER", Date::new(2024, 3, 14)),
        ("es", "+1s", Date::new(2024, 3, 22)),
        ("es", "+2m", Date::new(2024, 5, 15)),
        ("es", "+1a", Date::new(2025, 3, 15)),
        ("es", "+1 año", Date::new(2025, 3, 15)),
        ("ar", "غدا", Date::new(2024, 3, 16)),
        ("ar", "أمس", Date::new(2024, 3, 14)),
        ("ar", "+3 أيام", Date::new(2024, 3, 18)),
        ("ar", "+1ش", Date::new(2024, 4, 15)),
        ("ja", "明日", Date::new(2024, 3, 16)),
        ("hi", "कल", Date::new(2024, 3, 16)),
        ("hi", "बीता कल", Date::new(2024, 3, 14)),
    ];
    for (locale, input, expected) in cases {
        assert_eq!(
            Date::parse_input(input, locale, today, &words_for(locale)),
            expected,
            "{input} in {locale}"
        );
    }
    let now = Time::new(9, 30, 0).unwrap();
    assert_eq!(
        Time::parse_input("сейчас", "ru", now, &words_for("ru")),
        Some(now)
    );
    assert_eq!(
        Time::parse_input("现在", "zh", now, &words_for("zh")),
        Some(now)
    );
    assert_eq!(
        Time::parse_input("now", "ar", now, &words_for("ar")),
        Some(now)
    );
}

#[test]
fn unknown_words_and_other_digits_are_refused() {
    let today = Date::new(2024, 3, 15).unwrap();
    for (locale, input) in [
        ("ar", "+٣ي"),
        ("ru", "+3x"),
        ("zh", "后天"),
        ("es", "+3 lunas"),
        ("tr", "+3d3"),
    ] {
        assert_eq!(
            Date::parse_input(input, locale, today, &words_for(locale)),
            None,
            "{input} in {locale}"
        );
    }
}
