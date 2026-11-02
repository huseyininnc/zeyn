//! Dates and times without surprises.
//!
//! Four types, each with one meaning:
//!
//! | Type | Meaning | Stored as |
//! |---|---|---|
//! | [`Date`] | A calendar day, in no time zone | `2024-01-05` |
//! | [`Time`] | A wall clock time | `14:30:00` |
//! | [`DateTime`] | An exact moment | UTC, `2024-01-05T11:30:00.000Z` |
//! | [`LocalDateTime`] | A wall clock date and time at a place | `2024-01-05T14:30:00` |
//!
//! Only the server converts between a moment and a user's wall clock, using the user's
//! [`TimeZone`]. The browser only reports which time zone it is in.
//!
//! ```
//! use zeyn::{Date, DateTime, Time, TimeZone};
//! let istanbul = TimeZone::named("Europe/Istanbul").unwrap();
//! let moment = DateTime::parse("2024-01-05T11:30:00Z").unwrap();
//! assert_eq!(moment.local(istanbul).time, Time::new(14, 30, 0).unwrap());
//! assert_eq!(moment.local(istanbul).date, Date::new(2024, 1, 5).unwrap());
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

const MS_PER_DAY: i64 = 86_400_000;

/// A calendar day with no time zone: birth dates, invoice dates, holidays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u8) -> u8 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

impl Date {
    /// A valid date, or `None` for days such as February 30.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Date> {
        let valid = (1..=9999).contains(&year) && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        valid.then_some(Date { year, month, day })
    }

    pub fn year(&self) -> i32 {
        self.year
    }

    pub fn month(&self) -> u8 {
        self.month
    }

    pub fn day(&self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01 (negative before).
    pub fn days_since_epoch(&self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The date `days` after 1970-01-01.
    pub fn from_days_since_epoch(days: i64) -> Option<Date> {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        Date::new(i32::try_from(y).ok()?, m as u8, d as u8)
    }

    pub fn add_days(&self, days: i64) -> Option<Date> {
        Date::from_days_since_epoch(self.days_since_epoch().checked_add(days)?)
    }

    /// Day of the week, 1 = Monday ... 7 = Sunday.
    pub fn weekday(&self) -> u8 {
        ((self.days_since_epoch() + 3).rem_euclid(7) + 1) as u8
    }

    /// Reads `2024-01-05`.
    pub fn parse(s: &str) -> Option<Date> {
        let mut parts = s.trim().splitn(3, '-');
        let y = parts.next()?;
        let (m, d) = (parts.next()?, parts.next()?);
        if y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        Date::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    }

    /// Reads a date typed in the locale's short form (`05.01.2024` in `tr`, `1/5/2024` in `en-US`)
    /// or in ISO form.
    pub fn parse_local(s: &str, locale: &str) -> Option<Date> {
        if let Some(d) = Date::parse(s) {
            return Some(d);
        }
        let style = DateStyle::for_locale(locale);
        let parts: Vec<&str> = s.trim().split(['.', '/', '-']).collect();
        let [a, b, c] = parts[..] else { return None };
        let (d, m, y) = match style {
            DateStyle::DayMonthYear(_) => (a, b, c),
            DateStyle::MonthDayYear => (b, a, c),
            DateStyle::Iso => return None,
        };
        if y.len() != 4 {
            return None;
        }
        Date::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?)
    }

    /// Short form for a locale: `05.01.2024` in `tr`, `1/5/2024` in `en-US`, `05/01/2024` in `en-GB`.
    pub fn format(&self, locale: &str) -> String {
        match DateStyle::for_locale(locale) {
            DateStyle::DayMonthYear(sep) => format!("{:02}{sep}{:02}{sep}{}", self.day, self.month, self.year),
            DateStyle::MonthDayYear => format!("{}/{}/{}", self.month, self.day, self.year),
            DateStyle::Iso => self.to_string(),
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy)]
enum DateStyle {
    DayMonthYear(char),
    MonthDayYear,
    Iso,
}

impl DateStyle {
    fn for_locale(locale: &str) -> Self {
        let lower = locale.to_ascii_lowercase();
        let primary = lower.split(['-', '_']).next().unwrap_or("");
        match primary {
            "tr" | "de" | "ru" | "pl" | "cs" | "fi" | "nb" | "uk" | "az" => DateStyle::DayMonthYear('.'),
            "fr" | "es" | "it" | "pt" | "el" | "ar" => DateStyle::DayMonthYear('/'),
            "nl" => DateStyle::DayMonthYear('-'),
            "en" if lower == "en" || lower.ends_with("us") => DateStyle::MonthDayYear,
            "en" => DateStyle::DayMonthYear('/'),
            _ => DateStyle::Iso,
        }
    }
}

fn uses_12_hour(locale: &str) -> bool {
    let lower = locale.to_ascii_lowercase();
    lower == "en" || lower == "en-us"
}

/// A wall clock time with second precision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
    hour: u8,
    minute: u8,
    second: u8,
}

impl Time {
    pub fn new(hour: u8, minute: u8, second: u8) -> Option<Time> {
        (hour < 24 && minute < 60 && second < 60).then_some(Time { hour, minute, second })
    }

    pub fn hour(&self) -> u8 {
        self.hour
    }

    pub fn minute(&self) -> u8 {
        self.minute
    }

    pub fn second(&self) -> u8 {
        self.second
    }

    fn seconds(&self) -> i64 {
        i64::from(self.hour) * 3600 + i64::from(self.minute) * 60 + i64::from(self.second)
    }

    /// Reads `14:30` or `14:30:15`.
    pub fn parse(s: &str) -> Option<Time> {
        let parts: Vec<&str> = s.trim().split(':').collect();
        let num = |p: &str| (p.len() == 2).then(|| p.parse::<u8>().ok()).flatten();
        match parts[..] {
            [h, m] => Time::new(num(h)?, num(m)?, 0),
            [h, m, s] => Time::new(num(h)?, num(m)?, num(s)?),
            _ => None,
        }
    }

    /// `14:30` in most locales, `2:30 PM` in `en-US`. Seconds are shown only when not zero.
    pub fn format(&self, locale: &str) -> String {
        let secs = if self.second > 0 { format!(":{:02}", self.second) } else { String::new() };
        if uses_12_hour(locale) {
            let h = match self.hour % 12 {
                0 => 12,
                h => h,
            };
            let half = if self.hour < 12 { "AM" } else { "PM" };
            format!("{h}:{:02}{secs} {half}", self.minute)
        } else {
            format!("{:02}:{:02}{secs}", self.hour, self.minute)
        }
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

/// A date and wall clock time at a place, with no time zone: a clinic appointment at 10:00 stays
/// at 10:00 for everyone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime {
    pub date: Date,
    pub time: Time,
}

impl LocalDateTime {
    pub fn new(date: Date, time: Time) -> Self {
        LocalDateTime { date, time }
    }

    /// Reads `2024-01-05T14:30` or `2024-01-05T14:30:15` (a space works instead of `T`).
    pub fn parse(s: &str) -> Option<LocalDateTime> {
        let (d, t) = s.trim().split_once(['T', ' '])?;
        Some(LocalDateTime { date: Date::parse(d)?, time: Time::parse(t)? })
    }

    pub fn format(&self, locale: &str) -> String {
        format!("{} {}", self.date.format(locale), self.time.format(locale))
    }

    fn epoch_ms(&self) -> i64 {
        self.date.days_since_epoch() * MS_PER_DAY + self.time.seconds() * 1000
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}T{}", self.date, self.time)
    }
}

/// An exact moment, always stored in UTC with millisecond precision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    unix_ms: i64,
}

/// Why a wall clock time could not become a moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalTimeError {
    /// The time does not exist in that zone because clocks moved forward (daylight saving).
    Skipped,
}

impl DateTime {
    pub const fn from_unix_ms(unix_ms: i64) -> Self {
        DateTime { unix_ms }
    }

    pub const fn unix_ms(&self) -> i64 {
        self.unix_ms
    }

    /// Current moment from the system clock. Prefer `Ctx::now`, which tests can fix.
    pub fn now() -> DateTime {
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        DateTime { unix_ms: ms }
    }

    pub fn add_ms(&self, ms: i64) -> DateTime {
        DateTime { unix_ms: self.unix_ms.saturating_add(ms) }
    }

    /// Wall clock date and time of this moment in a zone.
    pub fn local(&self, zone: TimeZone) -> LocalDateTime {
        let local = self.unix_ms + i64::from(zone.offset_minutes_at(*self)) * 60_000;
        let days = local.div_euclid(MS_PER_DAY);
        let secs = local.rem_euclid(MS_PER_DAY) / 1000;
        LocalDateTime {
            date: Date::from_days_since_epoch(days).unwrap_or_default(),
            time: Time { hour: (secs / 3600) as u8, minute: (secs / 60 % 60) as u8, second: (secs % 60) as u8 },
        }
    }

    /// The moment when a wall clock in `zone` shows `local`. When clocks move back and a time
    /// happens twice, the earlier moment is used.
    pub fn from_local(local: LocalDateTime, zone: TimeZone) -> Result<DateTime, LocalTimeError> {
        let wall = local.epoch_ms();
        let mut candidates: Vec<DateTime> = zone
            .possible_offsets()
            .iter()
            .map(|off| DateTime { unix_ms: wall - i64::from(*off) * 60_000 })
            .filter(|dt| dt.local(zone) == local)
            .collect();
        candidates.sort();
        candidates.first().copied().ok_or(LocalTimeError::Skipped)
    }

    /// Reads RFC 3339: `2024-01-05T11:30:00Z`, `2024-01-05T14:30:00+03:00`, with optional fractions.
    pub fn parse(s: &str) -> Option<DateTime> {
        let s = s.trim();
        let (body, offset) = if let Some(b) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
            (b, 0)
        } else {
            let idx = s.rfind(['+', '-']).filter(|i| *i > 10)?;
            let (b, off) = s.split_at(idx);
            let sign = if off.starts_with('-') { -1 } else { 1 };
            let (h, m) = off[1..].split_once(':')?;
            (b, sign * (h.parse::<i32>().ok()? * 60 + m.parse::<i32>().ok()?))
        };
        let (main, fraction) = match body.split_once('.') {
            Some((m, f)) => (m, f),
            None => (body, ""),
        };
        let local = LocalDateTime::parse(main)?;
        let ms: i64 = if fraction.is_empty() {
            0
        } else {
            if !fraction.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            format!("{:0<3}", &fraction[..fraction.len().min(3)]).parse().ok()?
        };
        Some(DateTime { unix_ms: local.epoch_ms() + ms - i64::from(offset) * 60_000 })
    }

    /// Wall clock text in a zone and locale: `05.01.2024 14:30` for `tr`.
    pub fn format(&self, zone: TimeZone, locale: &str) -> String {
        self.local(zone).format(locale)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let l = self.local(TimeZone::UTC);
        write!(f, "{}T{}.{:03}Z", l.date, l.time, self.unix_ms.rem_euclid(1000))
    }
}

/// Daylight saving rules a zone follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    None,
    /// European Union: last Sunday of March to last Sunday of October, 01:00 UTC.
    Eu,
    /// United States and Canada: second Sunday of March to first Sunday of November, 02:00 local.
    Us,
}

/// A time zone: name, standard offset and daylight saving rule.
///
/// Built in zones cover UTC, fixed offsets (`UTC+03:00`) and common zones with today's rules.
/// Past rule changes are not modeled; for example Turkey used daylight saving before 2016.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeZone {
    name: &'static str,
    standard_minutes: i32,
    rule: Rule,
}

const ZONES: &[(&str, i32, Rule)] = &[
    ("UTC", 0, Rule::None),
    ("Etc/UTC", 0, Rule::None),
    ("Europe/Istanbul", 180, Rule::None),
    ("Asia/Istanbul", 180, Rule::None),
    ("Europe/Moscow", 180, Rule::None),
    ("Europe/Minsk", 180, Rule::None),
    ("Asia/Riyadh", 180, Rule::None),
    ("Asia/Baghdad", 180, Rule::None),
    ("Asia/Qatar", 180, Rule::None),
    ("Asia/Dubai", 240, Rule::None),
    ("Asia/Baku", 240, Rule::None),
    ("Asia/Tbilisi", 240, Rule::None),
    ("Asia/Tashkent", 300, Rule::None),
    ("Asia/Karachi", 300, Rule::None),
    ("Asia/Kolkata", 330, Rule::None),
    ("Asia/Almaty", 300, Rule::None),
    ("Asia/Bangkok", 420, Rule::None),
    ("Asia/Shanghai", 480, Rule::None),
    ("Asia/Singapore", 480, Rule::None),
    ("Asia/Tokyo", 540, Rule::None),
    ("Asia/Seoul", 540, Rule::None),
    ("Australia/Brisbane", 600, Rule::None),
    ("Africa/Cairo", 120, Rule::None),
    ("Africa/Johannesburg", 120, Rule::None),
    ("Africa/Lagos", 60, Rule::None),
    ("America/Sao_Paulo", -180, Rule::None),
    ("America/Phoenix", -420, Rule::None),
    ("Europe/London", 0, Rule::Eu),
    ("Europe/Dublin", 0, Rule::Eu),
    ("Europe/Lisbon", 0, Rule::Eu),
    ("Europe/Berlin", 60, Rule::Eu),
    ("Europe/Paris", 60, Rule::Eu),
    ("Europe/Amsterdam", 60, Rule::Eu),
    ("Europe/Brussels", 60, Rule::Eu),
    ("Europe/Vienna", 60, Rule::Eu),
    ("Europe/Zurich", 60, Rule::Eu),
    ("Europe/Rome", 60, Rule::Eu),
    ("Europe/Madrid", 60, Rule::Eu),
    ("Europe/Stockholm", 60, Rule::Eu),
    ("Europe/Oslo", 60, Rule::Eu),
    ("Europe/Copenhagen", 60, Rule::Eu),
    ("Europe/Warsaw", 60, Rule::Eu),
    ("Europe/Prague", 60, Rule::Eu),
    ("Europe/Budapest", 60, Rule::Eu),
    ("Europe/Belgrade", 60, Rule::Eu),
    ("Europe/Athens", 120, Rule::Eu),
    ("Europe/Bucharest", 120, Rule::Eu),
    ("Europe/Sofia", 120, Rule::Eu),
    ("Europe/Helsinki", 120, Rule::Eu),
    ("Europe/Kyiv", 120, Rule::Eu),
    ("Europe/Kiev", 120, Rule::Eu),
    ("Asia/Nicosia", 120, Rule::Eu),
    ("America/New_York", -300, Rule::Us),
    ("America/Toronto", -300, Rule::Us),
    ("America/Chicago", -360, Rule::Us),
    ("America/Denver", -420, Rule::Us),
    ("America/Los_Angeles", -480, Rule::Us),
    ("America/Vancouver", -480, Rule::Us),
];

/// Nth (1 based) or last (0) Sunday of a month.
fn sunday(year: i32, month: u8, nth: u8) -> Date {
    if nth == 0 {
        let last = Date::new(year, month, days_in_month(year, month)).unwrap_or_default();
        return last.add_days(-i64::from(last.weekday() % 7)).unwrap_or(last);
    }
    let first = Date::new(year, month, 1).unwrap_or_default();
    let to_sunday = (7 - i64::from(first.weekday())) % 7;
    first.add_days(to_sunday + 7 * i64::from(nth - 1)).unwrap_or(first)
}

impl TimeZone {
    pub const UTC: TimeZone = TimeZone { name: "UTC", standard_minutes: 0, rule: Rule::None };

    /// A built in zone by IANA name (`Europe/Istanbul`) or a fixed offset (`UTC+03:00`, `+05:30`).
    pub fn named(name: &str) -> Option<TimeZone> {
        if let Some(&(n, std, rule)) = ZONES.iter().find(|(n, ..)| n.eq_ignore_ascii_case(name)) {
            return Some(TimeZone { name: n, standard_minutes: std, rule });
        }
        let offset = name.strip_prefix("UTC").or_else(|| name.strip_prefix("GMT")).unwrap_or(name);
        let sign = match offset.chars().next()? {
            '+' => 1,
            '-' => -1,
            _ => return None,
        };
        let (h, m) = offset[1..].split_once(':').unwrap_or((&offset[1..], "0"));
        let (h, m): (i32, i32) = (h.parse().ok()?, m.parse().ok()?);
        if h > 14 || m > 59 {
            return None;
        }
        let minutes = sign * (h * 60 + m);
        let fixed: &'static [(&str, i32)] = &FIXED;
        let name = fixed.iter().find(|(_, mins)| *mins == minutes).map(|(n, _)| *n)?;
        Some(TimeZone { name, standard_minutes: minutes, rule: Rule::None })
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Names of the built in zones, for pickers.
    pub fn names() -> impl Iterator<Item = &'static str> {
        ZONES.iter().map(|(n, ..)| *n).filter(|n| !matches!(*n, "Etc/UTC" | "Asia/Istanbul" | "Europe/Kiev"))
    }

    /// Offset from UTC in minutes at a moment.
    pub fn offset_minutes_at(&self, at: DateTime) -> i32 {
        let dst = match self.rule {
            Rule::None => false,
            Rule::Eu => {
                let year = DateTime::from_unix_ms(at.unix_ms).local(TimeZone::UTC).date.year;
                let start = sunday(year, 3, 0).days_since_epoch() * MS_PER_DAY + 3_600_000;
                let end = sunday(year, 10, 0).days_since_epoch() * MS_PER_DAY + 3_600_000;
                (start..end).contains(&at.unix_ms)
            }
            Rule::Us => {
                let year = DateTime::from_unix_ms(at.unix_ms + i64::from(self.standard_minutes) * 60_000).local(TimeZone::UTC).date.year;
                let std_ms = i64::from(self.standard_minutes) * 60_000;
                let start = sunday(year, 3, 2).days_since_epoch() * MS_PER_DAY + 7_200_000 - std_ms;
                let end = sunday(year, 11, 1).days_since_epoch() * MS_PER_DAY + 7_200_000 - (std_ms + 3_600_000);
                (start..end).contains(&at.unix_ms)
            }
        };
        self.standard_minutes + if dst { 60 } else { 0 }
    }

    fn possible_offsets(&self) -> Vec<i32> {
        match self.rule {
            Rule::None => vec![self.standard_minutes],
            _ => vec![self.standard_minutes + 60, self.standard_minutes],
        }
    }

    /// Today's date in this zone at a moment.
    pub fn today(&self, now: DateTime) -> Date {
        now.local(*self).date
    }
}

static FIXED: [(&str, i32); 39] = [
    ("UTC-12:00", -720), ("UTC-11:00", -660), ("UTC-10:00", -600), ("UTC-09:30", -570), ("UTC-09:00", -540),
    ("UTC-08:00", -480), ("UTC-07:00", -420), ("UTC-06:00", -360), ("UTC-05:00", -300), ("UTC-04:00", -240),
    ("UTC-03:30", -210), ("UTC-03:00", -180), ("UTC-02:00", -120), ("UTC-01:00", -60), ("UTC", 0),
    ("UTC+01:00", 60), ("UTC+02:00", 120), ("UTC+03:00", 180), ("UTC+03:30", 210), ("UTC+04:00", 240),
    ("UTC+04:30", 270), ("UTC+05:00", 300), ("UTC+05:30", 330), ("UTC+05:45", 345), ("UTC+06:00", 360),
    ("UTC+06:30", 390), ("UTC+07:00", 420), ("UTC+08:00", 480), ("UTC+08:45", 525), ("UTC+09:00", 540),
    ("UTC+09:30", 570), ("UTC+10:00", 600), ("UTC+10:30", 630), ("UTC+11:00", 660), ("UTC+12:00", 720),
    ("UTC+12:45", 765), ("UTC+13:00", 780), ("UTC+14:00", 840), ("UTC+00:00", 0),
];

impl Default for TimeZone {
    fn default() -> Self {
        TimeZone::UTC
    }
}

macro_rules! text_serde {
    ($ty:ty, $parse:expr, $what:literal) => {
        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                $parse(&text).ok_or_else(|| serde::de::Error::custom(format!("invalid {}: `{text}`", $what)))
            }
        }
    };
}

text_serde!(Date, Date::parse, "date (expected YYYY-MM-DD)");
text_serde!(Time, Time::parse, "time (expected HH:MM or HH:MM:SS)");
text_serde!(LocalDateTime, LocalDateTime::parse, "local date time (expected YYYY-MM-DDTHH:MM:SS)");
text_serde!(DateTime, DateTime::parse, "date time (expected RFC 3339 such as 2024-01-05T11:30:00Z)");
