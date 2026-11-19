//! [`Heatmap`]: a number for each day of the last weeks, as a calendar of shaded squares.

use super::dom_id;
use super::layout::into_node;
use crate::grid::Tone;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use crate::time::Date;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

into_node!(Heatmap);

/// Most weeks a [`Heatmap`] shows: one year.
pub const MAX_HEATMAP_WEEKS: u8 = 53;

/// How many shades a [`Heatmap`] has above "nothing".
pub const HEATMAP_LEVELS: u32 = 4;

/// The shade of `value` when the largest value is `max`: 0 for nothing, then 1 to
/// [`HEATMAP_LEVELS`] in equal steps, so the largest value always gets the darkest shade.
///
/// ```
/// use zeyn::ui::heat_level;
/// assert_eq!(heat_level(0, 20), 0);
/// assert_eq!(heat_level(1, 20), 1);
/// assert_eq!(heat_level(5, 20), 1);
/// assert_eq!(heat_level(6, 20), 2);
/// assert_eq!(heat_level(20, 20), 4);
/// assert_eq!(heat_level(3, 0), 4);
/// ```
pub fn heat_level(value: u32, max: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    let max = u64::from(max.max(value));
    let level = (u64::from(value) * u64::from(HEATMAP_LEVELS)).div_ceil(max);
    level.clamp(1, u64::from(HEATMAP_LEVELS)) as u32
}

/// A number for each day of the last weeks, such as orders taken or hours logged, as a calendar
/// of squares: one column a week, one row a weekday, darker for more. The weeks end with the
/// week of `end`, usually today; days after `end` stay empty. The week starts on the day the
/// user's language starts it.
///
/// It is a table, so screen readers read the weekday of a row and, for each day, its date and
/// number in the user's language; the shade only repeats what the text says. The legend under
/// it runs from less to more. The table scrolls sideways inside its own box on narrow screens,
/// and the box takes focus so the keyboard can scroll it. Each day is two elements, so the
/// default of 17 weeks stays within the 300 element budget of a component and a year does not.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Heatmap;
///
/// let end = Date::new(2026, 11, 12).unwrap();
/// let orders = Heatmap::ending("Orders a day", end)
///     .weeks(12)
///     .value(end, 7)
///     .value(Date::new(2026, 11, 10).unwrap(), 3)
///     .tone(Tone::Success);
/// ```
pub struct Heatmap {
    label: Text,
    end: Date,
    weeks: u8,
    values: BTreeMap<i64, u32>,
    tone: Tone,
}

impl Heatmap {
    /// A heatmap named by `label` whose last week holds `end`.
    pub fn ending(label: impl Into<Text>, end: Date) -> Self {
        Heatmap {
            label: label.into(),
            end,
            weeks: 17,
            values: BTreeMap::new(),
            tone: Tone::Brand,
        }
    }

    /// How many weeks show, 1 to [`MAX_HEATMAP_WEEKS`]. The default is 17.
    pub fn weeks(mut self, weeks: u8) -> Self {
        self.weeks = weeks.clamp(1, MAX_HEATMAP_WEEKS);
        self
    }

    /// The number of a day. A day given twice keeps the last number; days outside the weeks
    /// shown are ignored when the heatmap renders.
    pub fn value(mut self, day: Date, value: u32) -> Self {
        let room = usize::from(MAX_HEATMAP_WEEKS) * 7;
        let key = day.days_since_epoch();
        if self.values.len() < room || self.values.contains_key(&key) {
            self.values.insert(key, value);
        }
        self
    }

    /// The numbers of many days at once.
    pub fn values(self, days: impl IntoIterator<Item = (Date, u32)>) -> Self {
        days.into_iter()
            .fold(self, |map, (day, value)| map.value(day, value))
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The first day of the first week shown, in the week of `locale`.
    fn start(&self, locale: &str) -> i64 {
        let first = Date::first_weekday(locale);
        let into_week = i64::from((self.end.weekday() + 7 - first) % 7);
        self.end.days_since_epoch() - into_week - 7 * (i64::from(self.weeks) - 1)
    }
}

impl Component for Heatmap {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "heat");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.heat.{key}")));
        let start = self.start(r.locale);
        let end = self.end.days_since_epoch();
        let max = self
            .values
            .range(start..=end)
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(0);
        out.reserve(usize::from(self.weeks) * 7 * 96);
        out.push_str(&format!(
            r#"<div class="z-heat z-tone-{}"><div class="z-heat-scroll" role="group" aria-labelledby="{id}" tabindex="0"><table class="z-heat-table"><caption class="z-heat-title" id="{id}">{}</caption><tbody>"#,
            self.tone.code(),
            r.text(&self.label)
        ));
        let first = Date::first_weekday(r.locale);
        for row in 0..7u8 {
            let weekday = (first - 1 + row) % 7 + 1;
            out.push_str(&format!(
                r#"<tr><th class="z-heat-day" scope="row">{}</th>"#,
                r.text(&Text::key(format!("zeyn.ui.week.days.{weekday}")))
            ));
            for week in 0..i64::from(self.weeks) {
                let day = start + week * 7 + i64::from(row);
                let date = (day <= end)
                    .then(|| Date::from_days_since_epoch(day))
                    .flatten();
                let Some(date) = date else {
                    out.push_str(r#"<td class="z-heat-off"></td>"#);
                    continue;
                };
                let value = self
                    .values
                    .get(&day)
                    .copied()
                    .unwrap_or(0);
                let text = r.text(
                    &Text::key("zeyn.ui.heat.day")
                        .arg("date", date.format(r.locale))
                        .arg("n", value.to_string()),
                );
                out.push_str(&format!(
                    r#"<td class="z-heat-cell z-heat-{}" title="{text}"><span class="z-sr-only">{text}</span></td>"#,
                    heat_level(value, max)
                ));
            }
            out.push_str("</tr>");
        }
        out.push_str(&format!(
            r#"</tbody></table></div><div class="z-heat-legend" aria-hidden="true"><span>{}</span>"#,
            t("less")
        ));
        for level in 0..=HEATMAP_LEVELS {
            out.push_str(&format!(
                r#"<span class="z-heat-cell z-heat-{level}"></span>"#
            ));
        }
        out.push_str(&format!("<span>{}</span></div></div>", t("more")));
    }

    fn schema(&self) -> Json {
        let days: Vec<Json> = self
            .values
            .iter()
            .filter_map(|(day, value)| {
                let date = Date::from_days_since_epoch(*day)?;
                Some(json!({ "day": date.to_string(), "value": value }))
            })
            .collect();
        json!({
            "type": "heatmap",
            "label": self.label,
            "end": self.end.to_string(),
            "weeks": self.weeks,
            "tone": self.tone,
            "days": days,
        })
    }
}
