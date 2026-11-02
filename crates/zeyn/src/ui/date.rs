//! Date and time editors: a text input in the page language's format with a custom popup.
//!
//! The browser never decides the format or the value. Inputs show and accept the short form of
//! the user's locale (`05.01.2024` in Turkish, `1/5/2024` in US English), plus natural input such
//! as `today`, `+3d` or `930` that the server reads ([`crate::Date::parse_input`]). The popup
//! follows the locale's first day of the week and month names. Without JavaScript the text input
//! still works; the server parses what was typed.

use crate::i18n::Text;
use crate::model::FieldMeta;
use crate::render::{Render, attr, escape};
use crate::time::{Date, DateTime, LocalDateTime, Time, TimeZone, range_presets};
use crate::value::{FieldKind, Value};
use serde::Serialize;
use serde_json::{Map, Value as Json, json};

/// Calendar and clock options of a date or time field. The picker uses them and the server checks
/// them again when the form is saved.
///
/// ```
/// use zeyn::ui::DateOptions;
/// let o = DateOptions::new().no_weekends().step_minutes(15).week_numbers();
/// assert_eq!(o.step(), 15);
/// ```
#[derive(Clone, Debug, Default, Serialize)]
pub struct DateOptions {
    no_weekends: bool,
    disabled: Vec<Date>,
    step: u16,
    seconds: bool,
    week_numbers: bool,
}

impl DateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Saturdays and Sundays cannot be chosen.
    pub fn no_weekends(mut self) -> Self {
        self.no_weekends = true;
        self
    }

    /// Days that cannot be chosen, such as holidays.
    pub fn disable(mut self, days: impl IntoIterator<Item = Date>) -> Self {
        self.disabled.extend(days);
        self
    }

    /// Minutes between choosable times, such as 5 or 15. 1 to 60; the default is 1.
    pub fn step_minutes(mut self, minutes: u16) -> Self {
        self.step = minutes.clamp(1, 60);
        self
    }

    /// Shows and accepts seconds.
    pub fn seconds(mut self) -> Self {
        self.seconds = true;
        self
    }

    /// Shows ISO week numbers in the calendar.
    pub fn week_numbers(mut self) -> Self {
        self.week_numbers = true;
        self
    }

    pub fn step(&self) -> u16 {
        self.step.max(1)
    }

    /// The catalog key of what is wrong with a value, or `None`.
    pub(crate) fn problem(&self, value: &Value, tz: TimeZone) -> Option<&'static str> {
        let (date, time) = match value {
            Value::Date(d) => (Some(*d), None),
            Value::Time(t) => (None, Some(*t)),
            Value::LocalDateTime(l) => (Some(l.date), Some(l.time)),
            Value::DateTime(m) => {
                let l = m.local(tz);
                (Some(l.date), Some(l.time))
            }
            _ => (None, None),
        };
        if let Some(d) = date {
            if (self.no_weekends && d.weekday() >= 6) || self.disabled.contains(&d) {
                return Some("zeyn.validation.disabled_day");
            }
        }
        if let Some(t) = time {
            let step = self.step();
            if step > 1 && (t.minute() % step as u8 != 0 || t.second() != 0) {
                return Some("zeyn.validation.time_step");
            }
        }
        None
    }
}

/// Whether `to` is before `from` for two values of the same kind. Empty values are never reversed.
pub(crate) fn range_reversed(from: &Value, to: &Value) -> bool {
    match (from, to) {
        (Value::Date(a), Value::Date(b)) => b < a,
        (Value::Time(a), Value::Time(b)) => b < a,
        (Value::LocalDateTime(a), Value::LocalDateTime(b)) => b < a,
        (Value::DateTime(a), Value::DateTime(b)) => b < a,
        _ => false,
    }
}

/// Kind name used by the browser: `date`, `time`, `datetime` (a moment, shown with its zone) or
/// `local` (a wall clock date and time).
pub(crate) fn kind_name(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Date => "date",
        FieldKind::Time => "time",
        FieldKind::DateTime => "datetime",
        _ => "local",
    }
}

/// The text a value shows in its input, in the user's language and zone.
pub(crate) fn input_text(value: &Value, r: &Render) -> String {
    match value {
        Value::Date(d) => d.format(r.locale),
        Value::Time(t) => t.format(r.locale),
        Value::LocalDateTime(l) => l.format(r.locale),
        Value::DateTime(d) => d.local(r.tz).format(r.locale),
        _ => String::new(),
    }
}

fn t(r: &Render, key: &str) -> String {
    r.i18n.text(r.locale, &Text::key(format!("zeyn.ui.date.{key}")))
}

/// Texts the popup needs, resolved in the user's language.
fn texts(r: &Render) -> Json {
    let mut map = Map::new();
    for key in [
        "dialog", "dialog_time", "dialog_range", "prev_month", "next_month", "prev_year", "next_year", "today", "now", "clear",
        "done", "month", "year", "months", "years", "utc", "hours", "minutes", "seconds", "period", "week", "week_long", "from", "to",
    ] {
        map.insert(key.to_string(), Json::String(t(r, key)));
    }
    map.insert("presets".into(), Json::String(t(r, "presets.label")));
    Json::Object(map)
}

/// Attributes shared by every picker root: locale pattern, limits, options and texts.
fn root_attrs(r: &Render, kind: &str, m: &FieldMeta, options: Option<&DateOptions>) -> String {
    let locale = r.locale;
    let (order, sep, pad) = Date::pattern(locale);
    let limit = |n: Option<i64>| n.and_then(Date::from_compact).map(|d| d.to_string()).unwrap_or_default();
    let today = r.tz.today(DateTime::now());
    let o = options.cloned().unwrap_or_default();
    let disabled: Vec<String> = o.disabled.iter().map(Date::to_string).collect();
    format!(
        r#"data-zeyn-date="{kind}" data-order="{order}" data-sep="{}" data-pad="{pad}" data-h12="{}" data-first-day="{}" data-min="{}" data-max="{}" data-today="{today}" data-disabled="{}" data-weekends="{}" data-weeks="{}" data-step="{}" data-seconds="{}" data-texts="{}""#,
        escape(&sep.to_string()),
        Time::twelve_hour(locale),
        Date::first_weekday(locale),
        limit(m.min()),
        limit(m.max()),
        disabled.join(","),
        if o.no_weekends { "off" } else { "on" },
        o.week_numbers,
        o.step(),
        o.seconds,
        attr(&texts(r)),
    )
}

/// Locale settings and texts for pickers the browser builds itself (grid filters), as
/// `data-*` names in camel case.
pub(crate) fn picker_config(r: &Render) -> Json {
    let (order, sep, pad) = Date::pattern(r.locale);
    json!({
        "data": {
            "order": order,
            "sep": sep.to_string(),
            "pad": pad.to_string(),
            "h12": Time::twelve_hour(r.locale).to_string(),
            "firstDay": Date::first_weekday(r.locale).to_string(),
            "today": r.tz.today(DateTime::now()).to_string(),
            "weekends": "on",
            "step": "1",
            "seconds": "false",
            "weeks": "false",
        },
        "texts": texts(r),
    })
}

/// The hint under an editor: the expected format with an example, and the words it accepts.
fn hint(r: &Render, kind: FieldKind, id: &str) -> String {
    let locale = r.locale;
    let day = Date::new(2024, 12, 31).unwrap_or_default();
    let time = Time::new(14, 30, 0).unwrap_or_default();
    let (sample, natural) = match kind {
        FieldKind::Date => (day.format(locale), "natural_date"),
        FieldKind::Time => (time.format(locale), "natural_time"),
        _ => (LocalDateTime::new(day, time).format(locale), "natural_date"),
    };
    let format = r.text(&Text::key("zeyn.ui.date.format").arg("format", sample));
    let words = r.text(&Text::key("zeyn.ui.date.natural").arg("examples", Text::key(format!("zeyn.ui.date.{natural}"))));
    format!(r#"<p class="z-field-hint" id="{id}-hint">{format} {words}</p>"#)
}

fn open_button(r: &Render, kind: &str, name: &str) -> String {
    let label = if kind == "time" { t(r, "open_time") } else { t(r, "open") };
    let icon = if kind == "time" { "z-date-icon z-time-icon" } else { "z-date-icon" };
    format!(
        r#"<button type="button" class="z-date-open" data-zeyn-date-open aria-haspopup="dialog" aria-expanded="false" aria-label="{}" data-testid="{}.field.{name}.picker"><span class="{icon}" aria-hidden="true"></span></button>"#,
        escape(&label),
        r.scope
    )
}

/// Writes the editor of a `Date`, `Time`, `DateTime` or `LocalDateTime` field. `common` holds the
/// input's id, name, test id and validation attributes; `id` is the input id.
pub(crate) fn render_date_input(
    out: &mut String,
    r: &Render,
    m: &FieldMeta,
    value: &Value,
    common: &str,
    id: &str,
    options: Option<&DateOptions>,
) {
    let kind = kind_name(m.kind);
    let zone = m.kind == FieldKind::DateTime;
    let described = if zone { format!("{id}-hint {id}-zone {id}-error") } else { format!("{id}-hint {id}-error") };
    let common = common.replace(&format!(r#"aria-describedby="{id}-error""#), &format!(r#"aria-describedby="{described}""#));
    out.push_str(&format!(r#"<div class="z-date" {}>"#, root_attrs(r, kind, m, options)));
    out.push_str(&format!(
        r#"<input type="text" autocomplete="off" spellcheck="false" data-zeyn-date-input {common} value="{}">"#,
        escape(&input_text(value, r))
    ));
    if zone {
        let utc = match value {
            Value::DateTime(d) => d.to_string(),
            _ => String::new(),
        };
        let title = if utc.is_empty() { String::new() } else { t(r, "utc").replace("{moment}", &utc) };
        out.push_str(&format!(
            r#"<span class="z-date-zone" id="{id}-zone" data-zeyn-zone title="{}"><span class="z-sr-only">{} </span>{}<span class="z-sr-only" data-zeyn-utc>{}</span></span>"#,
            escape(&title),
            escape(&t(r, "zone").replace("{zone}", "")),
            escape(r.tz.name()),
            if title.is_empty() { String::new() } else { format!(", {}", escape(&title)) },
        ));
    }
    out.push_str(&open_button(r, kind, m.name));
    out.push_str("</div>");
    out.push_str(&hint(r, m.kind, id));
}

/// Writes a from and to pair of date fields with one range picker and quick ranges.
pub(crate) fn render_range(out: &mut String, r: &Render, fields: [(&FieldMeta, Value); 2], options: Option<&DateOptions>) {
    let scope = r.scope;
    let today = r.tz.today(DateTime::now());
    let presets: Vec<Json> = range_presets(today)
        .into_iter()
        .map(|(key, from, to)| json!({ "key": key, "label": t(r, &format!("presets.{key}")), "from": from.to_string(), "to": to.to_string() }))
        .collect();
    let first = fields[0].0;
    out.push_str(&format!(
        r#"<div class="z-field z-range"><div class="z-date z-date-range" {} data-presets="{}">"#,
        root_attrs(r, "range", first, options),
        attr(&presets)
    ));
    for (i, (m, value)) in fields.iter().enumerate() {
        let id = format!("{scope}-{}", m.name);
        let required = if m.is_required() { r#" required aria-required="true""# } else { "" };
        let star = if m.is_required() { r#"<span class="z-required" aria-hidden="true">*</span>"# } else { "" };
        out.push_str(&format!(
            r#"<div class="z-range-part"><label for="{id}">{}{star}</label><input type="text" autocomplete="off" spellcheck="false" data-zeyn-date-input data-bound="{}" id="{id}" name="{}" data-testid="{scope}.field.{}" aria-describedby="{scope}-{}-hint {id}-error"{required} value="{}"></div>"#,
            r.label(m),
            if i == 0 { "from" } else { "to" },
            m.name,
            m.name,
            first.name,
            escape(&input_text(value, r)),
        ));
    }
    out.push_str(&open_button(r, "range", first.name));
    out.push_str("</div>");
    out.push_str(&hint(r, FieldKind::Date, &format!("{scope}-{}", first.name)));
    for (m, _) in &fields {
        out.push_str(&format!(r#"<p class="z-field-error" id="{scope}-{}-error" aria-live="polite"></p>"#, m.name));
    }
    out.push_str("</div>");
}
