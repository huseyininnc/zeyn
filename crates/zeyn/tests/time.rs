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
    assert_eq!(Date::new(1970, 1, 1).unwrap().days_since_epoch(), 0);
    assert_eq!(Date::new(2024, 2, 29).unwrap().weekday(), 4);
    assert!(Date::new(2023, 2, 29).is_none());
    assert!(Date::new(2100, 2, 29).is_none());
    assert!(Date::new(2000, 2, 29).is_some());
    assert_eq!(Date::new(2024, 12, 31).unwrap().add_days(1), Date::new(2025, 1, 1));
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
    assert_eq!(Time::new(14, 30, 0).unwrap().format("en-US"), "2:30 PM");
    assert_eq!(Time::new(0, 5, 0).unwrap().format("en-US"), "12:05 AM");
    assert_eq!(Time::new(14, 30, 0).unwrap().format("tr"), "14:30");
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
    assert_eq!(at("2024-01-05T11:30:00Z").to_string(), "2024-01-05T11:30:00.000Z");
    assert!(DateTime::parse("2024-01-05 11:30").is_none());
    let back: DateTime = serde_json::from_str("\"2024-01-05T11:30:00Z\"").unwrap();
    assert_eq!(back, at("2024-01-05T11:30:00Z"));
}

#[test]
fn istanbul_is_utc_plus_three_all_year() {
    let ist = tz("Europe/Istanbul");
    assert_eq!(at("2024-01-05T11:30:00Z").local(ist), local("2024-01-05T14:30:00"));
    assert_eq!(at("2024-07-05T11:30:00Z").local(ist), local("2024-07-05T14:30:00"));
    assert_eq!(at("2024-01-05T22:30:00Z").local(ist).date, Date::new(2024, 1, 6).unwrap());
}

#[test]
fn european_daylight_saving_switches_at_one_utc() {
    let berlin = tz("Europe/Berlin");
    assert_eq!(berlin.offset_minutes_at(at("2024-03-31T00:59:59Z")), 60);
    assert_eq!(berlin.offset_minutes_at(at("2024-03-31T01:00:00Z")), 120);
    assert_eq!(berlin.offset_minutes_at(at("2024-10-27T00:59:59Z")), 120);
    assert_eq!(berlin.offset_minutes_at(at("2024-10-27T01:00:00Z")), 60);
    assert_eq!(tz("Europe/London").offset_minutes_at(at("2024-06-01T12:00:00Z")), 60);
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
    assert_eq!(DateTime::from_local(local("2024-03-31T02:30:00"), berlin), Err(zeyn::LocalTimeError::Skipped));
    assert_eq!(DateTime::from_local(local("2024-10-27T02:30:00"), berlin), Ok(at("2024-10-27T00:30:00Z")));
    assert_eq!(DateTime::from_local(local("2024-06-01T10:00:00"), berlin), Ok(at("2024-06-01T08:00:00Z")));
    let ny = tz("America/New_York");
    assert_eq!(DateTime::from_local(local("2024-03-10T02:15:00"), ny), Err(zeyn::LocalTimeError::Skipped));
    assert_eq!(DateTime::from_local(local("2024-11-03T01:30:00"), ny), Ok(at("2024-11-03T05:30:00Z")));
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
    assert_eq!(ctx.with_time_zone(tz("Europe/Istanbul")).today(), Date::new(2024, 1, 6).unwrap());
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
    assert!(out.contains(r#"type="date" id="appt-day""#) && out.contains(r#"value="2024-01-05""#));
    assert!(out.contains(r#"type="time" step="1" id="appt-starts""#) && out.contains(r#"value="09:00:00""#));
    assert!(out.contains(r#"id="appt-booked_at""#) && out.contains(r#"value="2024-01-05T09:00:00""#));
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
    let valid = form().form().read(values.as_object().unwrap(), "tr", tz("Europe/Istanbul")).unwrap();
    assert_eq!(valid.day, Date::new(2024, 1, 6).unwrap());
    assert_eq!(valid.starts, Time::new(10, 15, 0).unwrap());
    assert_eq!(valid.booked_at, at("2024-01-05T09:00:00Z"));
    assert_eq!(valid.at_clinic, local("2024-01-06T10:15:00"));
    assert_eq!(valid.follow_up, None);

    let exact = serde_json::json!({ "booked_at": "2024-01-05T12:00:00Z" });
    let valid = form().form().read(exact.as_object().unwrap(), "tr", tz("Europe/Istanbul")).unwrap();
    assert_eq!(valid.booked_at, at("2024-01-05T12:00:00Z"));
}

#[test]
fn form_rejects_times_skipped_by_daylight_saving() {
    let values = serde_json::json!({ "booked_at": "2024-03-31T02:30", "day": "bad" });
    let errors = form().form().read(values.as_object().unwrap(), "en", tz("Europe/Berlin")).unwrap_err();
    let messages: Vec<String> = errors.errors.iter().map(|e| e.message.english()).collect();
    assert!(messages.contains(&"Day has an invalid value.".to_string()), "{messages:?}");
    assert!(messages.iter().any(|m| m.contains("does not exist in your time zone")), "{messages:?}");
}

#[test]
fn grid_sorts_dates_chronologically() {
    let rows = vec![
        Appointment { day: Date::new(2024, 3, 1).unwrap(), ..Default::default() },
        Appointment { day: Date::new(2023, 12, 31).unwrap(), ..Default::default() },
        Appointment { day: Date::new(2024, 1, 15).unwrap(), ..Default::default() },
    ];
    let q: GridQuery = serde_json::from_str(r#"{"sort":[{"field":"day","desc":false}]}"#).unwrap();
    let days: Vec<String> = q.apply(rows).items.iter().map(|a| a.day.to_string()).collect();
    assert_eq!(days, ["2023-12-31", "2024-01-15", "2024-03-01"]);
}
