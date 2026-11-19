//! Cron expressions with seconds in the Quartz dialect, and the next tick of one in a time zone.
//!
//! Six fields (second, minute, hour, day of month, month, day of week) or seven (with the
//! year). Each field takes `*`, numbers, lists (`1,15`), ranges (`MON-FRI`, `22-2` wraps) and
//! steps (`*/10`, `5/15`, `10-40/10`). The day of month also takes `?`, `L`, `L-3`, `15W` and
//! `LW`; the day of week takes `?`, names or `1` to `7` (1 is Sunday, as in Quartz), `6L` (the
//! last Friday) and `2#1` (the first Monday). One of the two day fields must be `*` or `?`.
//!
//! Ticks are wall clock times in the trigger's zone. A tick that falls in a daylight saving gap
//! runs once at the first instant after the gap; a tick in a fold runs once, at its first
//! occurrence, unless the hour field is `*` or a step, which follows the real clock and so
//! matches in both passes.

use crate::time::{Date, days_in_month};
use jiff::Timestamp;
use jiff::tz::{AmbiguousOffset, TimeZone as Zone};

/// First year a cron expression can name.
pub const MIN_YEAR: i32 = 1970;

/// Last year a cron expression can name; no tick is computed after it.
pub const MAX_YEAR: i32 = 2199;

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Days {
    Any,
    Set(u32),
    Last(u8),
    Nearest(u8),
    LastWeekday,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Weekdays {
    Any,
    Set(u8),
    Nth(u8, u8),
    Last(u8),
}

/// A parsed cron expression: each field compiled to a bit set, so finding the next tick
/// allocates nothing.
///
/// ```
/// use zeyn::metronome::Cron;
/// assert!(Cron::parse("0 30 7 ? * MON-FRI").is_ok());
/// let five = Cron::parse("30 7 * * *").unwrap_err();
/// assert!(five.contains("`0 30 7 * * *`"), "{five}");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cron {
    seconds: u64,
    minutes: u64,
    hours: u32,
    days: Days,
    months: u16,
    weekdays: Weekdays,
    years: Option<[u64; 4]>,
    hour_follows_clock: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Civil {
    date: Date,
    second: u32,
}

impl Civil {
    fn from_secs(secs: i64) -> Option<Civil> {
        let date = Date::from_days_since_epoch(secs.div_euclid(86_400))?;
        Some(Civil {
            date,
            second: secs.rem_euclid(86_400) as u32,
        })
    }

    fn secs(&self) -> i64 {
        self.date.days_since_epoch() * 86_400 + i64::from(self.second)
    }

    fn at(year: i32, month: u8, day: u8, second: u32) -> Option<Civil> {
        Some(Civil {
            date: Date::new(year, month, day)?,
            second,
        })
    }

    fn hour(&self) -> u32 {
        self.second / 3600
    }

    fn minute(&self) -> u32 {
        self.second / 60 % 60
    }

    fn sec(&self) -> u32 {
        self.second % 60
    }

    fn plus(&self, secs: i64) -> Option<Civil> {
        Civil::from_secs(self.secs() + secs)
    }

    fn next_day(&self) -> Option<Civil> {
        self.plus(86_400 - i64::from(self.second))
    }
}

fn next_bit(bits: u64, after: u32) -> Option<u32> {
    let rest = bits.checked_shr(after + 1).unwrap_or(0);
    (rest != 0).then(|| after + 1 + rest.trailing_zeros())
}

fn value(text: &str, min: u32, max: u32, names: &[&str], what: &str) -> Result<u32, String> {
    if let Some(i) = names
        .iter()
        .position(|n| n.eq_ignore_ascii_case(text))
    {
        return Ok(min + i as u32);
    }
    match text.parse::<u32>() {
        Ok(v) if (min..=max).contains(&v) => Ok(v),
        _ => Err(format!(
            "`{text}` is not a valid {what}; use {min} to {max}{}",
            if names.is_empty() {
                String::new()
            } else {
                format!(" or {} to {}", names[0], names[names.len() - 1])
            }
        )),
    }
}

/// Calls `set` with every value a field names.
fn fill(
    field: &str,
    min: u32,
    max: u32,
    names: &[&str],
    what: &str,
    set: &mut dyn FnMut(u32),
) -> Result<(), String> {
    if field.is_empty() {
        return Err(format!("the {what} field is empty"));
    }
    for part in field.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => {
                let step = s
                    .parse::<u32>()
                    .ok()
                    .filter(|s| (1..=max - min + 1).contains(s))
                    .ok_or_else(|| {
                        format!(
                            "`{s}` is not a valid step for the {what}; use 1 to {}",
                            max - min + 1
                        )
                    })?;
                (r, Some(step))
            }
            None => (part, None),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (
                value(a, min, max, names, what)?,
                value(b, min, max, names, what)?,
            )
        } else {
            let v = value(range, min, max, names, what)?;
            (v, if step.is_some() { max } else { v })
        };
        let step = step.unwrap_or(1);
        let span = if lo <= hi {
            hi - lo + 1
        } else {
            (max - lo + 1) + (hi - min + 1)
        };
        let mut i = 0;
        while i < span {
            let v = lo + i;
            set(if v > max { v - max - 1 + min } else { v });
            i += step;
        }
    }
    Ok(())
}

fn bits(field: &str, min: u32, max: u32, names: &[&str], what: &str) -> Result<u64, String> {
    let mut out = 0u64;
    fill(field, min, max, names, what, &mut |v| out |= 1 << v)?;
    Ok(out)
}

fn days(field: &str) -> Result<Days, String> {
    let upper = field.to_ascii_uppercase();
    Ok(match upper.as_str() {
        "*" | "?" => Days::Any,
        "L" => Days::Last(0),
        "LW" => Days::LastWeekday,
        _ if upper.starts_with("L-") => Days::Last(
            upper[2..]
                .parse::<u8>()
                .ok()
                .filter(|n| *n <= 30)
                .ok_or_else(|| format!("`{field}` is not a valid day; write `L-1` to `L-30`"))?,
        ),
        _ if upper.ends_with('W') => {
            Days::Nearest(value(&upper[..upper.len() - 1], 1, 31, &[], "day")? as u8)
        }
        _ => Days::Set(bits(field, 1, 31, &[], "day")? as u32),
    })
}

fn weekdays(field: &str) -> Result<Weekdays, String> {
    let upper = field.to_ascii_uppercase();
    let one = |t: &str| value(t, 1, 7, &WEEKDAYS, "weekday").map(|v| (v - 1) as u8);
    Ok(match upper.as_str() {
        "*" | "?" => Weekdays::Any,
        "L" => Weekdays::Set(1 << 6),
        _ if upper.len() > 1 && upper.ends_with('L') => {
            Weekdays::Last(one(&upper[..upper.len() - 1])?)
        }
        _ if upper.contains('#') => {
            let (d, n) = upper
                .split_once('#')
                .unwrap_or_default();
            let n = n
                .parse::<u8>()
                .ok()
                .filter(|n| (1..=5).contains(n))
                .ok_or_else(|| format!("`{field}` is not valid; after `#` write 1 to 5, such as `MON#1` for the first Monday"))?;
            Weekdays::Nth(one(d)?, n)
        }
        _ => {
            if upper
                .split([',', '-', '/'])
                .any(|p| p == "0")
            {
                return Err(format!(
                    "`{field}` uses 0 for a weekday; Zeyn follows Quartz, where 1 is Sunday and 7 is Saturday, or write names such as MON-FRI"
                ));
            }
            Weekdays::Set(bits(field, 1, 7, &WEEKDAYS, "weekday")?.wrapping_shr(1) as u8)
        }
    })
}

fn nearest_weekday(year: i32, month: u8, day: u8) -> u8 {
    let last = days_in_month(year, month);
    let day = day.min(last);
    let Some(date) = Date::new(year, month, day) else {
        return day;
    };
    match date.weekday() {
        6 if day == 1 => 3,
        6 => day - 1,
        7 if day == last => day - 2,
        7 => day + 1,
        _ => day,
    }
}

impl Cron {
    /// Reads an expression. Errors say what to change, such as adding the seconds field to a
    /// five field expression.
    pub fn parse(expr: &str) -> Result<Cron, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        match fields.len() {
            5 => {
                return Err(format!(
                    "`{expr}` has five fields, but Zeyn cron starts with the seconds; write `0 {}` to run at second 0",
                    fields.join(" ")
                ));
            }
            6 | 7 => {}
            n => {
                return Err(format!(
                    "`{expr}` has {n} fields; write six (second minute hour day month weekday) or seven (with the year), such as `0 30 7 ? * MON-FRI`"
                ));
            }
        }
        let years = match fields.get(6) {
            Some(f) if *f != "*" && *f != "?" => {
                let mut y = [0u64; 4];
                fill(f, MIN_YEAR as u32, MAX_YEAR as u32, &[], "year", &mut |v| {
                    let i = v - MIN_YEAR as u32;
                    y[(i / 64) as usize] |= 1 << (i % 64);
                })?;
                Some(y)
            }
            _ => None,
        };
        let cron = Cron {
            seconds: bits(fields[0], 0, 59, &[], "second")?,
            minutes: bits(fields[1], 0, 59, &[], "minute")?,
            hours: bits(fields[2], 0, 23, &[], "hour")? as u32,
            days: days(fields[3])?,
            months: bits(fields[4], 1, 12, &MONTHS, "month")? as u16,
            weekdays: weekdays(fields[5])?,
            years,
            hour_follows_clock: fields[2] == "*" || fields[2].contains('/'),
        };
        if cron.days != Days::Any && cron.weekdays != Weekdays::Any {
            return Err(format!(
                "`{expr}` sets both the day of the month and the weekday; put `?` in one of them, such as `0 0 8 ? * MON` or `0 0 8 1 * ?`"
            ));
        }
        Ok(cron)
    }

    fn year_ok(&self, year: i32) -> bool {
        let Some(y) = &self.years else {
            return true;
        };
        let i = year - MIN_YEAR;
        (0..256).contains(&i) && y[(i / 64) as usize] >> (i % 64) & 1 == 1
    }

    fn day_ok(&self, d: Date) -> bool {
        let (y, m, day) = (d.year(), d.month(), d.day());
        let last = days_in_month(y, m);
        let by_day = match self.days {
            Days::Any => true,
            Days::Set(bits) => bits >> day & 1 == 1,
            Days::Last(off) => last.checked_sub(off) == Some(day),
            Days::Nearest(n) => nearest_weekday(y, m, n) == day,
            Days::LastWeekday => nearest_weekday(y, m, last) == day,
        };
        let weekday = d.weekday() % 7;
        let by_weekday = match self.weekdays {
            Weekdays::Any => true,
            Weekdays::Set(bits) => bits >> weekday & 1 == 1,
            Weekdays::Nth(w, n) => w == weekday && (day - 1) / 7 + 1 == n,
            Weekdays::Last(w) => w == weekday && day + 7 > last,
        };
        by_day && by_weekday
    }

    /// The first wall clock time at or after `c` that matches, ignoring time zones.
    fn next_civil(&self, mut c: Civil) -> Option<Civil> {
        for _ in 0..200_000 {
            let (y, m) = (c.date.year(), c.date.month());
            if y > MAX_YEAR {
                return None;
            }
            if y < MIN_YEAR {
                c = Civil::at(MIN_YEAR, 1, 1, 0)?;
                continue;
            }
            if !self.year_ok(y) {
                let next = (y + 1..=MAX_YEAR).find(|y| self.year_ok(*y))?;
                c = Civil::at(next, 1, 1, 0)?;
                continue;
            }
            if self.months >> m & 1 == 0 {
                c = match next_bit(u64::from(self.months), u32::from(m)) {
                    Some(n) => Civil::at(y, n as u8, 1, 0)?,
                    None => Civil::at(y + 1, 1, 1, 0)?,
                };
                continue;
            }
            if !self.day_ok(c.date) {
                c = c.next_day()?;
                continue;
            }
            if self.hours >> c.hour() & 1 == 0 {
                c = match next_bit(u64::from(self.hours), c.hour()) {
                    Some(h) => Civil {
                        date: c.date,
                        second: h * 3600,
                    },
                    None => c.next_day()?,
                };
                continue;
            }
            if self.minutes >> c.minute() & 1 == 0 {
                c = match next_bit(self.minutes, c.minute()) {
                    Some(mi) => Civil {
                        date: c.date,
                        second: c.hour() * 3600 + mi * 60,
                    },
                    None => c.plus(i64::from(3600 - c.second % 3600))?,
                };
                continue;
            }
            if self.seconds >> c.sec() & 1 == 0 {
                c = match next_bit(self.seconds, c.sec()) {
                    Some(s) => Civil {
                        date: c.date,
                        second: c.second - c.sec() + s,
                    },
                    None => c.plus(i64::from(60 - c.sec()))?,
                };
                continue;
            }
            return Some(c);
        }
        None
    }

    /// The first tick strictly after `after_ms` (Unix milliseconds) in `zone`, as Unix
    /// milliseconds on a whole second, or `None` when the expression matches no later time
    /// before the year [`MAX_YEAR`] ends.
    ///
    /// ```
    /// use zeyn::{TimeZone, metronome::Cron};
    /// let istanbul = TimeZone::named("Europe/Istanbul").unwrap();
    /// let at_0730 = Cron::parse("0 30 7 * * *").unwrap();
    /// let midnight_utc = 1_767_225_600_000;
    /// assert_eq!(at_0730.next_after(midnight_utc, istanbul), Some(midnight_utc + 4 * 3_600_000 + 1_800_000));
    /// ```
    pub fn next_after(&self, after_ms: i64, zone: crate::TimeZone) -> Option<i64> {
        self.next_in(after_ms, &zone.jiff())
    }

    /// [`Cron::next_after`] with the zone already converted.
    pub(crate) fn next_in(&self, after_ms: i64, zone: &Zone) -> Option<i64> {
        let mut u = after_ms.div_euclid(1000) + 1;
        for _ in 0..256 {
            let off = offset(zone, u);
            let local = Civil::from_secs(u + off)?;
            let c = self.next_civil(local)?;
            let delta = c.secs() - local.secs();
            if delta == 0 {
                if self.hour_follows_clock {
                    return Some(u * 1000);
                }
                match second_pass_end(zone, u, off, local) {
                    Some(end) => {
                        u = end;
                        continue;
                    }
                    None => return Some(u * 1000),
                }
            }
            let candidate = u + delta;
            match next_transition(zone, u) {
                Some(t) if t <= candidate => {
                    let new = offset(zone, t);
                    if new > off && c.secs() < t + new {
                        return Some(t * 1000);
                    }
                    u = t;
                }
                _ => u = candidate,
            }
        }
        None
    }
}

fn offset(zone: &Zone, secs: i64) -> i64 {
    Timestamp::from_second(secs)
        .map(|t| i64::from(zone.to_offset(t).seconds()))
        .unwrap_or(0)
}

fn next_transition(zone: &Zone, secs: i64) -> Option<i64> {
    let t = Timestamp::from_second(secs).ok()?;
    zone.following(t)
        .next()
        .map(|tr| tr.timestamp().as_second())
}

/// When `u` is in the second pass of a fold (the wall clock repeats an hour), the instant the
/// second pass ends.
fn second_pass_end(zone: &Zone, u: i64, off: i64, local: Civil) -> Option<i64> {
    let civil = jiff::civil::DateTime::new(
        local.date.year() as i16,
        local.date.month() as i8,
        local.date.day() as i8,
        local.hour() as i8,
        local.minute() as i8,
        local.sec() as i8,
        0,
    )
    .ok()?;
    match zone
        .to_ambiguous_timestamp(civil)
        .offset()
    {
        AmbiguousOffset::Fold { before, after }
            if i64::from(after.seconds()) == off && before != after =>
        {
            let t = Timestamp::from_second(u + 1).ok()?;
            let start = zone
                .preceding(t)
                .next()?
                .timestamp()
                .as_second();
            Some(start + i64::from(before.seconds() - after.seconds()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first matching second after `after_ms`, found by testing every second of the days
    /// whose date matches, over `days` days.
    fn brute(cron: &Cron, after_ms: i64, days: i64) -> Option<i64> {
        let start = after_ms.div_euclid(1000) + 1;
        let first = start.div_euclid(86_400);
        (first..first + days)
            .filter(|d| {
                let date = Date::from_days_since_epoch(*d).unwrap();
                cron.year_ok(date.year())
                    && cron.months >> date.month() & 1 == 1
                    && cron.day_ok(date)
            })
            .flat_map(|d| d * 86_400..(d + 1) * 86_400)
            .find(|s| {
                let c = Civil::from_secs(*s).unwrap();
                *s >= start
                    && cron.hours >> c.hour() & 1 == 1
                    && cron.minutes >> c.minute() & 1 == 1
                    && cron.seconds >> c.sec() & 1 == 1
            })
            .map(|s| s * 1000)
    }

    #[test]
    fn next_matches_a_second_by_second_search_in_utc() {
        let utc = Zone::UTC;
        let exprs = [
            "*/7 * * * * *",
            "0 */13 * * * *",
            "15 10 3 * * ?",
            "0 0 0 L * ?",
            "0 0 12 15W * ?",
            "0 0 9 ? * 6L",
            "0 0 9 ? * MON#2",
            "0 0 0 LW * ?",
            "0 0 22-2 ? * SAT,SUN",
            "0 0 0 L-2 FEB ?",
            "0 0 0 29 FEB ? 2028/4",
            "0 15,45 8-17/3 ? JAN-MAR,OCT MON-FRI",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for expr in exprs {
            let cron = Cron::parse(expr).unwrap();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let after = 1_700_000_000_000 + (seed % 400_000_000_000) as i64;
                let fast = cron.next_in(after, &utc);
                assert!(fast.is_some_and(|n| n > after), "{expr}");
                assert_eq!(fast, brute(&cron, after, 3000), "{expr} after {after}");
            }
        }
    }
}
