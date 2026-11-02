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
        let valid =
            (1..=9999).contains(&year) && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
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

    /// The date as the number `YYYYMMDD`, the form date limits are written in: `2024_01_05`.
    pub fn compact(&self) -> i64 {
        i64::from(self.year) * 10_000 + i64::from(self.month) * 100 + i64::from(self.day)
    }

    /// The date of a `YYYYMMDD` number, or `None` when it is not a valid day.
    pub fn from_compact(n: i64) -> Option<Date> {
        Date::new(i32::try_from(n / 10_000).ok()?, (n / 100 % 100) as u8, (n % 100) as u8)
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

    /// How a locale writes short dates: the order of the parts (`dmy`, `mdy` or `ymd`), the
    /// separator and whether day and month are padded to two digits. Used by the date picker.
    pub fn pattern(locale: &str) -> (&'static str, char, bool) {
        match DateStyle::for_locale(locale) {
            DateStyle::DayMonthYear(sep) => ("dmy", sep, true),
            DateStyle::MonthDayYear => ("mdy", '/', false),
            DateStyle::Iso => ("ymd", '-', true),
        }
    }

    /// The first day of the week in a locale: 7 (Sunday) for `en-US`, 1 (Monday) elsewhere.
    pub fn first_weekday(locale: &str) -> u8 {
        if uses_12_hour(locale) { 7 } else { 1 }
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

/// Words people type instead of a date or time, in one language: `bugün`, `yarın`, `+3g` in
/// Turkish, `today`, `tomorrow`, `+3d` in English. Read from the `zeyn.date.words.*` catalog keys,
/// each a comma separated list. English words are always understood as well.
#[derive(Clone, Debug, Default)]
pub struct DateWords {
    today: Vec<String>,
    tomorrow: Vec<String>,
    yesterday: Vec<String>,
    now: Vec<String>,
    /// Units for relative input, in the order day, week, month, year.
    units: [Vec<String>; 4],
}

impl DateWords {
    /// Words of a locale from the catalogs, followed by the English ones.
    pub fn from_i18n(i18n: &crate::i18n::I18n, locale: &str) -> DateWords {
        let list = |key: &str| -> Vec<String> {
            let mut words: Vec<String> = Vec::new();
            for l in [locale, "en"] {
                if let Some(text) = i18n.lookup(l, &format!("zeyn.date.words.{key}")) {
                    for w in text.split(',').map(|w| lower(w.trim())).filter(|w| !w.is_empty()) {
                        if !words.contains(&w) {
                            words.push(w);
                        }
                    }
                }
            }
            words
        };
        DateWords {
            today: list("today"),
            tomorrow: list("tomorrow"),
            yesterday: list("yesterday"),
            now: list("now"),
            units: [list("day"), list("week"), list("month"), list("year")],
        }
    }

    /// English words only, from the built in catalog.
    pub fn english() -> DateWords {
        DateWords::from_i18n(crate::i18n::I18n::builtin(), "en")
    }

    fn is(list: &[String], word: &str) -> bool {
        list.iter().any(|w| w == word)
    }

    /// Unit index of a word: 0 day, 1 week, 2 month, 3 year. The locale's words come first, so
    /// `h` is a week in Turkish (hafta).
    fn unit(&self, word: &str) -> Option<usize> {
        if word.is_empty() {
            return Some(0);
        }
        self.units.iter().position(|list| DateWords::is(list, word))
    }
}

/// Lower case that keeps Turkish dotted and dotless i apart: `İ` becomes `i`, `I` becomes `ı`
/// only when the text has other Turkish letters; plain ASCII words stay ASCII.
fn lower(s: &str) -> String {
    let turkish = s.chars().any(|c| "çğıöşüÇĞİÖŞÜ".contains(c));
    s.chars()
        .flat_map(|c| match c {
            'İ' => vec!['i'],
            'I' if turkish => vec!['ı'],
            c => c.to_lowercase().collect(),
        })
        .collect()
}

/// Splits relative input such as `+3g`, `- 2 weeks` or `+1` into sign, amount and unit word.
fn relative(s: &str) -> Option<(i64, String)> {
    let s = s.trim();
    let (sign, rest) = match s.chars().next()? {
        '+' => (1, &s[1..]),
        '-' => (-1, &s[1..]),
        _ => return None,
    };
    let rest = rest.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let unit = lower(rest[digits.len()..].trim());
    Some((sign * digits.parse::<i64>().ok()?, unit))
}

impl Date {
    /// The date `months` months after this one, on the same day or the last day of a shorter month.
    pub fn add_months(&self, months: i64) -> Option<Date> {
        let index = i64::from(self.year) * 12 + i64::from(self.month) - 1 + months;
        let year = i32::try_from(index.div_euclid(12)).ok()?;
        let month = (index.rem_euclid(12) + 1) as u8;
        Date::new(year, month, self.day.min(days_in_month(year, month)))
    }

    /// Reads what a person types into a date field. Besides the full forms of [`Date::parse_local`]:
    ///
    /// - Partial dates in the locale's order: `5` (day of this month), `5.1` or `1/5` (this year),
    ///   `5.1.24` (two digit year in 2000 to 2099), digits only such as `05012024`.
    /// - Words: `today`, `tomorrow`, `yesterday` and their translations (`bugün`, `yarın`, `dün`).
    /// - Relative input: `+3d`, `-1w`, `+2m`, `+1y` (`+3g`, `-1h`, `+2a`, `+1y` in Turkish); a bare
    ///   number is days.
    ///
    /// ```
    /// use zeyn::{Date, DateWords};
    /// let today = Date::new(2024, 3, 15).unwrap();
    /// let tr = DateWords::from_i18n(zeyn::I18n::builtin(), "tr");
    /// assert_eq!(Date::parse_input("5.1", "tr", today, &tr), Date::new(2024, 1, 5));
    /// assert_eq!(Date::parse_input("yarın", "tr", today, &tr), Date::new(2024, 3, 16));
    /// assert_eq!(Date::parse_input("+1h", "tr", today, &tr), Date::new(2024, 3, 22));
    /// assert_eq!(Date::parse_input("1/5", "en-US", today, &DateWords::english()), Date::new(2024, 1, 5));
    /// ```
    pub fn parse_input(s: &str, locale: &str, today: Date, words: &DateWords) -> Option<Date> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if let Some(d) = Date::parse_local(s, locale) {
            return Some(d);
        }
        let word = lower(s);
        if DateWords::is(&words.today, &word) {
            return Some(today);
        }
        if DateWords::is(&words.tomorrow, &word) {
            return today.add_days(1);
        }
        if DateWords::is(&words.yesterday, &word) {
            return today.add_days(-1);
        }
        if let Some((n, unit)) = relative(s) {
            return match words.unit(&unit)? {
                0 => today.add_days(n),
                1 => today.add_days(n * 7),
                2 => today.add_months(n),
                _ => today.add_months(n * 12),
            };
        }
        let (order, _, _) = Date::pattern(locale);
        let parts: Vec<&str> = if s.bytes().all(|b| b.is_ascii_digit()) {
            match (s.len(), order) {
                (1 | 2, _) => vec![s],
                (4, "ymd") => vec![&s[..2], &s[2..]],
                (4, _) => vec![&s[..2], &s[2..]],
                (6, "ymd") => vec![&s[..2], &s[2..4], &s[4..]],
                (6, _) => vec![&s[..2], &s[2..4], &s[4..]],
                (8, "ymd") => vec![&s[..4], &s[4..6], &s[6..]],
                (8, _) => vec![&s[..2], &s[2..4], &s[4..]],
                _ => return None,
            }
        } else {
            s.split(['.', '/', '-', ' ']).filter(|p| !p.is_empty()).collect()
        };
        if parts.iter().any(|p| !p.bytes().all(|b| b.is_ascii_digit()) || p.len() > 4) {
            return None;
        }
        let num = |p: &str| p.parse::<i64>().ok();
        let year = |p: &str| match p.len() {
            2 => num(p).map(|y| 2000 + y),
            4 => num(p),
            _ => None,
        };
        let (y, m, d) = match (order, parts.as_slice()) {
            (_, [d]) => (i64::from(today.year), i64::from(today.month), num(d)?),
            ("mdy", [m, d]) => (i64::from(today.year), num(m)?, num(d)?),
            ("ymd", [m, d]) => (i64::from(today.year), num(m)?, num(d)?),
            (_, [d, m]) => (i64::from(today.year), num(m)?, num(d)?),
            ("mdy", [m, d, y]) => (year(y)?, num(m)?, num(d)?),
            ("ymd", [y, m, d]) => (year(y)?, num(m)?, num(d)?),
            (_, [d, m, y]) => (year(y)?, num(m)?, num(d)?),
            _ => return None,
        };
        Date::new(i32::try_from(y).ok()?, u8::try_from(m).ok()?, u8::try_from(d).ok()?)
    }
}

/// Ranges offered next to a date range picker, worked out from today in the user's time zone:
/// `(key, from, to)` with keys `today`, `yesterday`, `last_7_days`, `this_month`, `last_month`,
/// `this_year`. Labels are the `zeyn.ui.date.presets.{key}` catalog texts.
pub fn range_presets(today: Date) -> Vec<(&'static str, Date, Date)> {
    let first = Date::new(today.year, today.month, 1).unwrap_or(today);
    let last_month_first = first.add_months(-1).unwrap_or(first);
    let jan = Date::new(today.year, 1, 1).unwrap_or(today);
    [
        ("today", Some(today), Some(today)),
        ("yesterday", today.add_days(-1), today.add_days(-1)),
        ("last_7_days", today.add_days(-6), Some(today)),
        ("this_month", Some(first), Date::new(today.year, today.month, days_in_month(today.year, today.month))),
        ("last_month", Some(last_month_first), first.add_days(-1)),
        ("this_year", Some(jan), Date::new(today.year, 12, 31)),
    ]
    .into_iter()
    .filter_map(|(k, a, b)| Some((k, a?, b?)))
    .collect()
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

    /// Whether a locale shows times on a 12 hour clock with AM and PM (`en-US`).
    pub fn twelve_hour(locale: &str) -> bool {
        uses_12_hour(locale)
    }

    /// Reads what a person types into a time field. Besides [`Time::parse_local`]: `14` (on the
    /// hour), `1430` and `930` (digits only), `14.30`, `2pm`, and `now` or its translation (`şimdi`).
    ///
    /// ```
    /// use zeyn::{DateWords, Time};
    /// let now = Time::new(10, 7, 0).unwrap();
    /// let en = DateWords::english();
    /// assert_eq!(Time::parse_input("930", "tr", now, &en), Time::new(9, 30, 0));
    /// assert_eq!(Time::parse_input("2pm", "en-US", now, &en), Time::new(14, 0, 0));
    /// assert_eq!(Time::parse_input("now", "en", now, &en), Some(now));
    /// ```
    pub fn parse_input(s: &str, locale: &str, now: Time, words: &DateWords) -> Option<Time> {
        let s = s.trim();
        if DateWords::is(&words.now, &lower(s)) {
            return Some(now);
        }
        if let Some(t) = Time::parse_local(&s.replace('.', ":"), locale) {
            return Some(t);
        }
        let upper = s.to_ascii_uppercase().replace(' ', "");
        let (digits, half) = match (upper.strip_suffix("AM"), upper.strip_suffix("PM")) {
            (Some(d), _) => (d.to_string(), Some(0)),
            (_, Some(d)) => (d.to_string(), Some(12)),
            _ => (upper, None),
        };
        if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let (h, m) = match digits.len() {
            1 | 2 => (digits.parse::<u8>().ok()?, 0),
            3 => (digits[..1].parse::<u8>().ok()?, digits[1..].parse::<u8>().ok()?),
            _ => (digits[..2].parse::<u8>().ok()?, digits[2..].parse::<u8>().ok()?),
        };
        let hour = match half {
            Some(add) if (1..=12).contains(&h) => h % 12 + add,
            Some(_) => return None,
            None => h,
        };
        Time::new(hour, m, 0)
    }

    /// Reads a time as typed in any locale: `14:30`, `14:30:15`, `9:05`, `2:30 PM` or `2:30 pm`.
    pub fn parse_local(s: &str, _locale: &str) -> Option<Time> {
        let s = s.trim();
        let upper = s.to_ascii_uppercase();
        let (clock, half) = match (upper.strip_suffix("AM"), upper.strip_suffix("PM")) {
            (Some(c), _) => (c.trim_end().to_string(), Some(0)),
            (_, Some(c)) => (c.trim_end().to_string(), Some(12)),
            _ => (upper, None),
        };
        let parts: Vec<&str> = clock.split(':').collect();
        let num = |p: &str| {
            (!p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse::<u8>().ok())
                .flatten()
        };
        let (h, m, sec) = match parts[..] {
            [h, m] => (num(h)?, num(m)?, 0),
            [h, m, x] => (num(h)?, num(m)?, num(x)?),
            _ => return None,
        };
        if parts[1].len() != 2 {
            return None;
        }
        let hour = match half {
            Some(add) if (1..=12).contains(&h) => h % 12 + add,
            Some(_) => return None,
            None => h,
        };
        Time::new(hour, m, sec)
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

    /// Reads a date and time typed in the locale's short form (`05.01.2024 14:30` in `tr`,
    /// `1/5/2024 2:30 PM` in `en-US`) or in ISO form.
    pub fn parse_local(s: &str, locale: &str) -> Option<LocalDateTime> {
        if let Some(l) = LocalDateTime::parse(s) {
            return Some(l);
        }
        let (d, t) = s.trim().split_once(['T', ' '])?;
        Some(LocalDateTime { date: Date::parse_local(d, locale)?, time: Time::parse_local(t, locale)? })
    }

    /// Reads what a person types into a date and time field: a date as [`Date::parse_input`]
    /// reads it, optionally followed by a time as [`Time::parse_input`] reads it (`yarın 9`,
    /// `+1d 14:30`, `5.1 930`). A date alone means midnight; `now` is the current wall clock.
    pub fn parse_input(s: &str, locale: &str, now: LocalDateTime, words: &DateWords) -> Option<LocalDateTime> {
        let s = s.trim();
        if let Some(l) = LocalDateTime::parse_local(s, locale) {
            return Some(l);
        }
        if DateWords::is(&words.now, &lower(s)) {
            return Some(now);
        }
        if let Some(date) = Date::parse_input(s, locale, now.date, words) {
            return Some(LocalDateTime { date, time: Time::default() });
        }
        let (d, t) = s.split_once(['T', ' '])?;
        Some(LocalDateTime {
            date: Date::parse_input(d, locale, now.date, words)?,
            time: Time::parse_input(t, locale, now.time, words)?,
        })
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
        let local = self.unix_ms + i64::from(zone.offset_seconds_at(*self)) * 1000;
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
        let offset = zone.offset_seconds_of(local)?;
        Ok(DateTime { unix_ms: local.epoch_ms() - i64::from(offset) * 1000 })
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

/// A time zone: an IANA zone such as `Europe/Istanbul` with its whole history of offset and
/// daylight saving changes, or a fixed offset such as `UTC+03:00`.
///
/// Zone data is the IANA time zone database bundled into the binary (through `jiff`), so every
/// platform gives the same answers, Windows included, without a system zoneinfo. A zone is read
/// from the bundle the first time its name is used and then kept for the life of the process
/// (a few kilobytes each, at most one copy per zone). `TimeZone` itself is a small `Copy` handle.
///
/// ```
/// use zeyn::{DateTime, TimeZone};
/// let istanbul = TimeZone::named("Europe/Istanbul").unwrap();
/// assert_eq!(istanbul.offset_minutes_at(DateTime::parse("2015-07-01T00:00:00Z").unwrap()), 180);
/// assert_eq!(istanbul.offset_minutes_at(DateTime::parse("2015-12-01T00:00:00Z").unwrap()), 120);
/// assert_eq!(istanbul.offset_minutes_at(DateTime::parse("2016-12-01T00:00:00Z").unwrap()), 180);
/// ```
#[derive(Clone, Copy)]
pub struct TimeZone {
    name: &'static str,
    zone: Zone,
}

#[derive(Clone, Copy)]
enum Zone {
    /// Offset in minutes.
    Fixed(i32),
    Iana(&'static jiff::tz::TimeZone),
}

impl PartialEq for TimeZone {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for TimeZone {}

impl fmt::Debug for TimeZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TimeZone").field(&self.name).finish()
    }
}

/// Common zones offered by pickers, in the order shown.
const PICKER: &[&str] = &[
    "UTC",
    "Europe/Istanbul",
    "Europe/Moscow",
    "Europe/Minsk",
    "Asia/Riyadh",
    "Asia/Baghdad",
    "Asia/Qatar",
    "Asia/Dubai",
    "Asia/Baku",
    "Asia/Tbilisi",
    "Asia/Tashkent",
    "Asia/Karachi",
    "Asia/Kolkata",
    "Asia/Almaty",
    "Asia/Bangkok",
    "Asia/Shanghai",
    "Asia/Singapore",
    "Asia/Tokyo",
    "Asia/Seoul",
    "Australia/Brisbane",
    "Africa/Cairo",
    "Africa/Johannesburg",
    "Africa/Lagos",
    "America/Sao_Paulo",
    "America/Phoenix",
    "Europe/London",
    "Europe/Dublin",
    "Europe/Lisbon",
    "Europe/Berlin",
    "Europe/Paris",
    "Europe/Amsterdam",
    "Europe/Brussels",
    "Europe/Vienna",
    "Europe/Zurich",
    "Europe/Rome",
    "Europe/Madrid",
    "Europe/Stockholm",
    "Europe/Oslo",
    "Europe/Copenhagen",
    "Europe/Warsaw",
    "Europe/Prague",
    "Europe/Budapest",
    "Europe/Belgrade",
    "Europe/Athens",
    "Europe/Bucharest",
    "Europe/Sofia",
    "Europe/Helsinki",
    "Europe/Kyiv",
    "Asia/Nicosia",
    "America/New_York",
    "America/Toronto",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Vancouver",
];

/// Zones read from the bundled database so far. Only names the database knows are added, so
/// the list is bounded by the number of IANA zones whatever clients send.
static LOADED: std::sync::Mutex<Vec<TimeZone>> = std::sync::Mutex::new(Vec::new());

impl TimeZone {
    pub const UTC: TimeZone = TimeZone { name: "UTC", zone: Zone::Fixed(0) };

    /// A zone by IANA name (`Europe/Istanbul`, any case) or a fixed offset (`UTC+03:00`, `+05:30`).
    pub fn named(name: &str) -> Option<TimeZone> {
        if let Some(fixed) = TimeZone::fixed(name) {
            return Some(fixed);
        }
        if name.eq_ignore_ascii_case("UTC") {
            return Some(TimeZone::UTC);
        }
        if name.is_empty() || name.len() > 64 {
            return None;
        }
        let mut loaded = LOADED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(z) = loaded.iter().find(|z| z.name.eq_ignore_ascii_case(name)) {
            return Some(*z);
        }
        let tz = jiff::tz::db().get(name).ok()?;
        let canonical = tz.iana_name()?;
        if let Some(z) = loaded.iter().find(|z| z.name == canonical) {
            return Some(*z);
        }
        let name: &'static str = Box::leak(canonical.into());
        let zone = TimeZone { name, zone: Zone::Iana(Box::leak(Box::new(tz.clone()))) };
        loaded.push(zone);
        Some(zone)
    }

    fn fixed(name: &str) -> Option<TimeZone> {
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
        let name = FIXED.iter().find(|(_, mins)| *mins == minutes).map(|(n, _)| *n)?;
        Some(TimeZone { name, zone: Zone::Fixed(minutes) })
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Names of common zones, for pickers. [`TimeZone::named`] accepts every IANA name.
    pub fn names() -> impl Iterator<Item = &'static str> {
        PICKER.iter().copied()
    }

    /// Offset from UTC in minutes at a moment. Historical local mean time offsets with seconds
    /// are rounded toward zero; [`DateTime::local`] keeps the seconds.
    pub fn offset_minutes_at(&self, at: DateTime) -> i32 {
        self.offset_seconds_at(at) / 60
    }

    fn offset_seconds_at(&self, at: DateTime) -> i32 {
        match self.zone {
            Zone::Fixed(minutes) => minutes * 60,
            Zone::Iana(tz) => {
                let ms = at.unix_ms.clamp(jiff::Timestamp::MIN.as_millisecond(), jiff::Timestamp::MAX.as_millisecond());
                jiff::Timestamp::from_millisecond(ms).map(|t| tz.to_offset(t).seconds()).unwrap_or(0)
            }
        }
    }

    /// Offset in seconds of a wall clock time; the earlier moment's when the time happens twice.
    fn offset_seconds_of(&self, local: LocalDateTime) -> Result<i32, LocalTimeError> {
        let Zone::Iana(tz) = self.zone else { return Ok(self.offset_seconds_at(DateTime::default())) };
        let (d, t) = (local.date, local.time);
        let civil = jiff::civil::DateTime::new(
            d.year as i16,
            d.month as i8,
            d.day as i8,
            t.hour as i8,
            t.minute as i8,
            t.second as i8,
            0,
        )
        .map_err(|_| LocalTimeError::Skipped)?;
        match tz.to_ambiguous_timestamp(civil).offset() {
            jiff::tz::AmbiguousOffset::Unambiguous { offset } => Ok(offset.seconds()),
            jiff::tz::AmbiguousOffset::Gap { .. } => Err(LocalTimeError::Skipped),
            jiff::tz::AmbiguousOffset::Fold { before, after } => Ok(before.seconds().max(after.seconds())),
        }
    }

    /// Today's date in this zone at a moment.
    pub fn today(&self, now: DateTime) -> Date {
        now.local(*self).date
    }
}

static FIXED: [(&str, i32); 39] = [
    ("UTC-12:00", -720),
    ("UTC-11:00", -660),
    ("UTC-10:00", -600),
    ("UTC-09:30", -570),
    ("UTC-09:00", -540),
    ("UTC-08:00", -480),
    ("UTC-07:00", -420),
    ("UTC-06:00", -360),
    ("UTC-05:00", -300),
    ("UTC-04:00", -240),
    ("UTC-03:30", -210),
    ("UTC-03:00", -180),
    ("UTC-02:00", -120),
    ("UTC-01:00", -60),
    ("UTC", 0),
    ("UTC+01:00", 60),
    ("UTC+02:00", 120),
    ("UTC+03:00", 180),
    ("UTC+03:30", 210),
    ("UTC+04:00", 240),
    ("UTC+04:30", 270),
    ("UTC+05:00", 300),
    ("UTC+05:30", 330),
    ("UTC+05:45", 345),
    ("UTC+06:00", 360),
    ("UTC+06:30", 390),
    ("UTC+07:00", 420),
    ("UTC+08:00", 480),
    ("UTC+08:45", 525),
    ("UTC+09:00", 540),
    ("UTC+09:30", 570),
    ("UTC+10:00", 600),
    ("UTC+10:30", 630),
    ("UTC+11:00", 660),
    ("UTC+12:00", 720),
    ("UTC+12:45", 765),
    ("UTC+13:00", 780),
    ("UTC+14:00", 840),
    ("UTC+00:00", 0),
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
