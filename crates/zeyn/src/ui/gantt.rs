//! [`Gantt`]: tasks as bars over a span of days.

use super::dom_id;
use super::layout::into_node;
use crate::grid::Tone;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use crate::time::Date;
use serde_json::{Value as Json, json};

into_node!(Gantt);

/// Most tasks a [`Gantt`] holds.
pub const MAX_GANTT_TASKS: usize = 100;

/// Most days a [`Gantt`] spans: two years.
pub const MAX_GANTT_DAYS: i64 = 731;

struct Task {
    title: Text,
    from: Date,
    to: Date,
    done: u8,
    tone: Tone,
}

/// Tasks as bars over a span of days, such as the plan of a project. It is a table: each row
/// names the task and gives its first and last day as text, and the bar repeats them as a
/// picture. `.today(day)` draws a line at that day, and `.done(percent)` fills part of the bar
/// of the task added last. Bars are cut at the ends of the span.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Gantt;
/// let day = |d: u8| Date::new(2026, 11, d).unwrap();
/// let plan = Gantt::new("Launch plan", day(1), day(30))
///     .task("Design", day(2), day(9))
///     .done(100)
///     .task("Build", day(8), day(24))
///     .done(40)
///     .today(day(12));
/// ```
pub struct Gantt {
    label: Text,
    start: Date,
    end: Date,
    tasks: Vec<Task>,
    today: Option<Date>,
}

impl Gantt {
    /// A chart named by `label` from `start` to `end`, at most [`MAX_GANTT_DAYS`] days.
    pub fn new(label: impl Into<Text>, start: Date, end: Date) -> Self {
        let last = start
            .add_days(MAX_GANTT_DAYS - 1)
            .unwrap_or(start);
        let end = if end.days_since_epoch() < start.days_since_epoch() {
            start
        } else if end.days_since_epoch() > last.days_since_epoch() {
            last
        } else {
            end
        };
        Gantt {
            label: label.into(),
            start,
            end,
            tasks: Vec::new(),
            today: None,
        }
    }

    /// Adds a task from its first to its last day. At most [`MAX_GANTT_TASKS`].
    pub fn task(mut self, title: impl Into<Text>, from: Date, to: Date) -> Self {
        if self.tasks.len() < MAX_GANTT_TASKS {
            let to = if to.days_since_epoch() < from.days_since_epoch() {
                from
            } else {
                to
            };
            self.tasks.push(Task {
                title: title.into(),
                from,
                to,
                done: 0,
                tone: Tone::Brand,
            });
        }
        self
    }

    /// How much of the task added last is done, in percent.
    pub fn done(mut self, percent: u8) -> Self {
        if let Some(task) = self.tasks.last_mut() {
            task.done = percent.min(100);
        }
        self
    }

    /// The tone of the task added last.
    pub fn tone(mut self, tone: Tone) -> Self {
        if let Some(task) = self.tasks.last_mut() {
            task.tone = tone;
        }
        self
    }

    /// Draws a line at this day when it is within the span.
    pub fn today(mut self, day: Date) -> Self {
        self.today = Some(day);
        self
    }

    fn days(&self) -> i64 {
        self.end.days_since_epoch() - self.start.days_since_epoch() + 1
    }

    /// Where a day starts within the span, in hundredths of a percent, kept within it.
    fn at(&self, day: i64) -> i64 {
        let offset = (day - self.start.days_since_epoch()).clamp(0, self.days());
        offset * 10_000 / self.days()
    }
}

/// Hundredths of a percent as a CSS percentage.
fn percent(value: i64) -> String {
    format!("{}.{:02}%", value / 100, value % 100)
}

impl Component for Gantt {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "gantt");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.gantt.{key}")));
        out.push_str(&format!(
            r#"<div class="z-table-scroll" role="group" aria-labelledby="{id}" tabindex="0"><table class="z-gantt"><caption class="z-table-caption" id="{id}">{}</caption><thead><tr><th scope="col">{}</th><th scope="col">{}</th><th scope="col">{}</th><td class="z-gantt-track" aria-hidden="true">"#,
            r.text(&self.label),
            t("task"),
            t("from"),
            t("to")
        ));
        let mut month = Date::new(self.start.year(), self.start.month(), 1);
        while let Some(first) = month {
            if first.days_since_epoch() > self.end.days_since_epoch() {
                break;
            }
            if first.days_since_epoch() >= self.start.days_since_epoch()
                || first.month() == self.start.month()
            {
                let name = r.text(&Text::key(format!(
                    "zeyn.ui.calendar.months.{}",
                    first.month()
                )));
                out.push_str(&format!(
                    r#"<span class="z-gantt-month" style="--z-gantt-at:{}">{name}</span>"#,
                    percent(self.at(first.days_since_epoch()))
                ));
            }
            month = first.add_months(1);
        }
        out.push_str("</td></tr></thead><tbody>");
        let today = self
            .today
            .map(|d| d.days_since_epoch())
            .filter(|d| *d >= self.start.days_since_epoch() && *d <= self.end.days_since_epoch())
            .map(|d| {
                format!(
                    r#"<span class="z-gantt-today" style="--z-gantt-at:{}"></span>"#,
                    percent(self.at(d))
                )
            })
            .unwrap_or_default();
        for task in &self.tasks {
            let from = self.at(task.from.days_since_epoch());
            let to = self.at(task.to.days_since_epoch() + 1);
            let bar = if to > from {
                format!(
                    r#"<span class="z-gantt-bar z-tone-{}" style="--z-gantt-at:{};--z-gantt-width:{}"><span class="z-gantt-done" style="--z-gantt-done:{}%"></span></span>"#,
                    task.tone.code(),
                    percent(from),
                    percent(to - from),
                    task.done
                )
            } else {
                String::new()
            };
            let done = if task.done > 0 {
                format!(
                    r#"<span class="z-sr-only">, {}</span>"#,
                    r.text(&Text::key("zeyn.ui.gantt.done").arg("n", task.done.to_string()))
                )
            } else {
                String::new()
            };
            out.push_str(&format!(
                r#"<tr><th scope="row">{}{done}</th><td class="z-gantt-date">{}</td><td class="z-gantt-date">{}</td><td class="z-gantt-track" aria-hidden="true">{bar}{today}</td></tr>"#,
                r.text(&task.title),
                task.from.format(r.locale),
                task.to.format(r.locale)
            ));
        }
        out.push_str("</tbody></table></div>");
    }

    fn schema(&self) -> Json {
        let tasks: Vec<Json> = self
            .tasks
            .iter()
            .map(|t| json!({ "title": t.title, "from": t.from.to_string(), "to": t.to.to_string(), "done": t.done, "tone": t.tone }))
            .collect();
        json!({
            "type": "gantt",
            "label": self.label,
            "start": self.start.to_string(),
            "end": self.end.to_string(),
            "today": self.today.map(|d| d.to_string()),
            "tasks": tasks,
        })
    }
}
