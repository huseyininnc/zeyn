//! [`Calendar`]: events of an action in a month, week or day, in the user's time zone.

use super::catalog::check_masks;
use super::widgets::screen_href;
use super::{DataSource, dom_id, topic_list, topics_attr};
use crate::action::{Action, ActionMeta, DataQuery, Error, Filter, PageRequest, Result, Sort};
use crate::ctx::{BoxFuture, Ctx};
use crate::grid::{FetchFn, fetch_via};
use crate::i18n::Text;
use crate::model::{Field, FieldError, Model, Rule, ValidationErrors};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Effect, GridSource, LoadedPage, Node, Params};
use crate::time::{Date, DateTime, LocalDateTime, Time, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

/// Most events one view loads.
pub const MAX_CALENDAR_EVENTS: u32 = 500;

/// Events shown in a month cell before a "more" note.
const MONTH_CELL_EVENTS: usize = 3;

/// The span a [`Calendar`] shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarView {
    /// Six weeks around a month, a cell per day.
    #[default]
    Month,
    /// Seven days in the user's week order, a row per hour.
    Week,
    /// One day, a row per hour.
    Day,
}

impl CalendarView {
    fn name(self) -> &'static str {
        match self {
            CalendarView::Month => "month",
            CalendarView::Week => "week",
            CalendarView::Day => "day",
        }
    }
}

/// Screen params of a calendar page: the view and the date it shows, both optional (this month
/// unless set). The calendar's toolbar changes them in place; a screen with more params uses
/// fields with the same names, `view` and `date`.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Calendar, CalendarParams};
///
/// #[screen(route = "/planner", public)]
/// pub fn planner(p: CalendarParams) -> Screen {
///     Screen::new("Planner").body(Paragraph::new(format!("{:?}", p.view)))
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CalendarParams {
    pub view: Option<CalendarView>,
    pub date: Option<Date>,
}

impl<'de> Deserialize<'de> for CalendarParams {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            view: Option<CalendarView>,
            #[serde(default)]
            date: Option<Date>,
        }
        Ok(Option::<Raw>::deserialize(d)?
            .map(|r| CalendarParams {
                view: r.view,
                date: r.date,
            })
            .unwrap_or_default())
    }
}

impl Params for CalendarParams {}

/// What the create action of a [`Calendar`] gets: the slot the user chose, as moments read in
/// the user's time zone on the server. `all_day` is set for days chosen in the month view or the
/// all day row; `end` is then the midnight after the last day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarSlot {
    pub start: DateTime,
    pub end: DateTime,
    pub all_day: bool,
}

/// What the browser sends when a slot is chosen: local dates (all day) or local date times.
#[derive(Deserialize)]
struct SlotCall {
    start: String,
    end: String,
    #[serde(default)]
    all_day: bool,
}

/// A validation error on the slot, written for the user.
fn slot_error(key: &str) -> Error {
    let mut errors = ValidationErrors::default();
    errors.errors.push(FieldError {
        field: "slot",
        rule: Rule::Custom,
        message: Text::key(format!("zeyn.ui.calendar.{key}")),
    });
    Error::Validation(errors)
}

/// Reads a slot from the browser in the user's zone: at most 31 days, start before end.
fn read_slot(call: &SlotCall, zone: TimeZone) -> Result<CalendarSlot> {
    let moment =
        |l: LocalDateTime| DateTime::from_local(l, zone).map_err(|_| slot_error("skipped"));
    let (start, end) = if call.all_day {
        let (Some(a), Some(b)) = (Date::parse(&call.start), Date::parse(&call.end)) else {
            return Err(Error::NotFound);
        };
        let after = b.add_days(1).ok_or(Error::NotFound)?;
        (
            moment(LocalDateTime::new(a, Time::default()))?,
            moment(LocalDateTime::new(after, Time::default()))?,
        )
    } else {
        let (Some(a), Some(b)) = (
            LocalDateTime::parse(&call.start),
            LocalDateTime::parse(&call.end),
        ) else {
            return Err(Error::NotFound);
        };
        (moment(a)?, moment(b)?)
    };
    let span = end.unix_ms() - start.unix_ms();
    if span <= 0 || span > 31 * 86_400_000 {
        return Err(slot_error("invalid_slot"));
    }
    Ok(CalendarSlot {
        start,
        end,
        all_day: call.all_day,
    })
}

/// The create action of a calendar: reads the slot, runs the action and sends back its effect.
struct CreateSource<A>(PhantomData<fn() -> A>);

impl<A> DataSource for CreateSource<A>
where
    A: Action<Input = CalendarSlot, Output = Result<Option<Effect>>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let call: SlotCall = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            if call.start.len() > 40 || call.end.len() > 40 {
                return Err(Error::NotFound);
            }
            let slot = read_slot(&call, ctx.time_zone())?;
            let effect = A::call(ctx.clone(), slot).await??;
            let r = Render::new("calendar", ctx.locale(), ctx.i18n());
            Ok(json!({ "effect": effect.map(|e| e.to_json(&r)) }))
        })
    }
}

type LabelFn<E> = Box<dyn Fn(&E) -> String + Send + Sync>;
type OpenFn<E> = Box<dyn Fn(&E) -> Effect + Send + Sync>;

/// One event placed in the view: local start and end, its row and its HTML.
struct Placed {
    start: LocalDateTime,
    end: LocalDateTime,
    html: String,
}

/// Events of an action in a month, week or day view, in the user's time zone. The action is the
/// kind a DataGrid uses (`DataQuery` in, `Page` out) and gets only the visible range: two
/// filters, the start field up to the last visible day and the end field from the first one
/// (dates are whole days in the user's zone), at most [`MAX_CALENDAR_EVENTS`] rows sorted by
/// start. Everything is rendered on the server; the toolbar's links change the screen's
/// [`CalendarParams`] in place through its topic (`"calendar"`), so switching the view or the
/// period is one request and the address can be shared.
///
/// The days are an APG grid: one tab stop, arrow keys move between days (or hours), Home and End
/// to the start and end of the row, Page Up and Page Down to the previous and next period, Enter
/// into the events of a cell (Escape back out). With [`Calendar::open`] an event opens its
/// window; with [`Calendar::create`] choosing a slot (a click, a drag over hours, Shift with the
/// arrow keys, then Enter or N) runs the action with a [`CalendarSlot`].
///
/// ```ignore
/// #[screen(route = "/planner", permission = "events.read")]
/// pub fn planner(p: CalendarParams) -> Screen {
///     Screen::new(t!("planner.title")).body(
///         Calendar::of(search_events)
///             .start(Event::starts)
///             .end(Event::ends)
///             .label(|e: &Event| e.title.clone())
///             .open(|e: &Event| Window::open(event, EventParams { id: e.id }))
///             .create(new_event)
///             .at(p),
///     )
/// }
/// ```
pub struct Calendar<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    start: Option<Field<E, DateTime>>,
    end: Option<Field<E, DateTime>>,
    label: Option<LabelFn<E>>,
    open: Option<OpenFn<E>>,
    create: Option<Box<dyn DataSource>>,
    view: CalendarView,
    date: Option<Date>,
    hours: (u8, u8),
    topics: Vec<&'static str>,
    title: Option<Text>,
}

impl<E: Model> Calendar<E> {
    pub fn of<A>(_source: A) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<crate::action::Page<E>>>,
    {
        Calendar {
            action: A::META,
            fetch: fetch_via::<A, E>,
            start: None,
            end: None,
            label: None,
            open: None,
            create: None,
            view: CalendarView::Month,
            date: None,
            hours: (7, 21),
            topics: vec!["calendar"],
            title: None,
        }
    }

    /// When an event starts.
    pub fn start(mut self, field: Field<E, DateTime>) -> Self {
        self.start = Some(field);
        self
    }

    /// When an event ends. Without it events are points at their start.
    pub fn end(mut self, field: Field<E, DateTime>) -> Self {
        self.end = Some(field);
        self
    }

    /// The text of an event.
    pub fn label(mut self, label: impl Fn(&E) -> String + Send + Sync + 'static) -> Self {
        self.label = Some(Box::new(label));
        self
    }

    /// What pressing an event does, such as `Window::open(event, EventParams { id: e.id })`.
    /// The window's screen checks its own access.
    pub fn open(mut self, open: impl Fn(&E) -> Effect + Send + Sync + 'static) -> Self {
        self.open = Some(Box::new(open));
        self
    }

    /// Creating an event by choosing a slot runs `action` with a [`CalendarSlot`] through
    /// `POST /_zeyn/data` (CSRF, write limit, the action's permission). It answers with an
    /// effect to run, such as a window with the new event's form, or `None`; the calendar then
    /// renders again. Users without the permission cannot choose slots.
    pub fn create<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = CalendarSlot, Output = Result<Option<Effect>>>,
    {
        self.create = Some(Box::new(CreateSource::<A>(PhantomData)));
        self
    }

    /// The view and date from the screen's params.
    pub fn at(mut self, params: CalendarParams) -> Self {
        self.view = params.view.unwrap_or_default();
        self.date = params.date;
        self
    }

    /// The view to show, month unless set.
    pub fn view(mut self, view: CalendarView) -> Self {
        self.view = view;
        self
    }

    /// A day in the period to show, today in the user's zone unless set.
    pub fn date(mut self, date: Date) -> Self {
        self.date = Some(date);
        self
    }

    /// The hours the week and day views show, `from` to `to` (0 to 24); 7 to 21 unless set.
    /// Events outside them are shown in the first or last row.
    pub fn hours(mut self, from: u8, to: u8) -> Self {
        let from = from.min(23);
        self.hours = (from, to.clamp(from + 1, 24));
        self
    }

    /// The topics after which the calendar renders again, `"calendar"` unless set. Its own
    /// toolbar drives the first one.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        let list = topic_list(topics);
        if !list.is_empty() {
            self.topics = list;
        }
        self
    }

    /// A heading over the calendar, before the period.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The day shown, the first and the last visible day.
    fn range(&self, today: Date, locale: &str) -> (Date, Date, Date) {
        let day = self.date.unwrap_or(today);
        let week_start = |d: Date| {
            let first = i64::from(Date::first_weekday(locale));
            let back = (i64::from(d.weekday()) - first).rem_euclid(7);
            d.add_days(-back).unwrap_or(d)
        };
        match self.view {
            CalendarView::Day => (day, day, day),
            CalendarView::Week => {
                let a = week_start(day);
                (day, a, a.add_days(6).unwrap_or(a))
            }
            CalendarView::Month => {
                let first = Date::new(day.year(), day.month(), 1).unwrap_or(day);
                let a = week_start(first);
                (day, a, a.add_days(41).unwrap_or(a))
            }
        }
    }

    /// The day the previous or next period starts from.
    fn step(&self, day: Date, forward: bool) -> Date {
        let sign = if forward { 1 } else { -1 };
        match self.view {
            CalendarView::Month => day.add_months(sign),
            CalendarView::Week => day.add_days(7 * sign),
            CalendarView::Day => day.add_days(sign),
        }
        .unwrap_or(day)
    }

    fn today(r: &Render) -> Date {
        r.ctx
            .map(|c| c.today())
            .unwrap_or_else(|| r.tz.today(DateTime::now()))
    }

    fn allow(&self, query: &mut DataQuery, today: Date, locale: &str) {
        let (_, first, last) = self.range(today, locale);
        let mut filters = Vec::new();
        if let Some(start) = self.start {
            filters.push(Filter {
                field: start.meta().name.to_string(),
                to: Some(last.to_string()),
                from: self
                    .end
                    .is_none()
                    .then(|| first.to_string()),
                ..Filter::default()
            });
        }
        if let Some(end) = self.end {
            filters.push(Filter {
                field: end.meta().name.to_string(),
                from: Some(first.to_string()),
                ..Filter::default()
            });
        }
        *query = DataQuery {
            page: PageRequest::Offset {
                offset: 0,
                limit: MAX_CALENDAR_EVENTS,
            },
            sort: self
                .start
                .iter()
                .map(|f| Sort {
                    field: f.meta().name.to_string(),
                    desc: false,
                })
                .collect(),
            filters,
            time_zone: query.time_zone,
            ..DataQuery::default()
        };
    }

    /// The events of a page with their local times and HTML.
    fn placed(&self, r: &Render, page: Option<&LoadedPage>) -> Vec<Placed> {
        let (Some(rows), Some(start)) = (
            page.and_then(|p| p.items.downcast_ref::<Vec<E>>()),
            self.start,
        ) else {
            return Vec::new();
        };
        rows.iter()
            .map(|row| {
                let a = *start.get(row);
                let b = self
                    .end
                    .map(|f| *f.get(row))
                    .filter(|b| *b > a)
                    .unwrap_or(a);
                let (la, lb) = (a.local(r.tz), b.local(r.tz));
                let label = self
                    .label
                    .as_ref()
                    .map(|l| l(row))
                    .unwrap_or_default();
                let time = escape(&Time::new(la.time.hour(), la.time.minute(), 0).unwrap_or_default().format(r.locale));
                let inner = format!(
                    r#"<time class="z-cal-time" datetime="{a}">{time}</time> <span class="z-cal-label">{}</span>"#,
                    escape(&label)
                );
                let html = match &self.open {
                    Some(open) => format!(
                        r#"<button type="button" class="z-cal-event" tabindex="-1" data-zeyn-effect="{}">{inner}</button>"#,
                        attr(&open(row).to_json(r))
                    ),
                    None => format!(r#"<span class="z-cal-event" tabindex="-1">{inner}</span>"#),
                };
                Placed {
                    start: la,
                    end: lb,
                    html,
                }
            })
            .collect()
    }

    /// The last day an event covers: an end at midnight belongs to the day before.
    fn last_day(p: &Placed) -> Date {
        if p.end.time == Time::default() && p.end.date > p.start.date {
            p.end
                .date
                .add_days(-1)
                .unwrap_or(p.end.date)
        } else {
            p.end.date
        }
    }

    fn day_name(r: &Render, d: Date) -> String {
        r.text(&Text::key(format!("zeyn.ui.week.days.{}", d.weekday())))
    }

    /// The title of the period: "November 2026", a week's first and last day, or a day.
    fn period(&self, r: &Render, day: Date, first: Date, last: Date) -> String {
        match self.view {
            CalendarView::Month => format!(
                "{} {}",
                r.text(&Text::key(format!(
                    "zeyn.ui.calendar.months.{}",
                    day.month()
                ))),
                day.year()
            ),
            CalendarView::Week => r.text(
                &Text::key("zeyn.ui.calendar.range")
                    .arg("from", first.format(r.locale))
                    .arg("to", last.format(r.locale)),
            ),
            CalendarView::Day => format!(
                "{} {}",
                Self::day_name(r, day),
                escape(&day.format(r.locale))
            ),
        }
    }

    /// A toolbar link that changes the view or the date in place.
    fn link(
        &self,
        r: &Render,
        view: CalendarView,
        date: Date,
        text: &str,
        current: bool,
    ) -> String {
        let params = json!({ "view": view.name(), "date": date.to_string() });
        format!(
            r#"<a class="z-cal-link" href="{}" data-zeyn-params="{}"{}>{text}</a>"#,
            screen_href(r, r.scope, &params),
            attr(&params),
            if current {
                r#" aria-current="page""#
            } else {
                ""
            }
        )
    }

    fn toolbar(&self, out: &mut String, r: &Render, (day, first, last): (Date, Date, Date)) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.calendar.{key}")));
        let id = dom_id(r, "cal");
        let today = Self::today(r);
        out.push_str(&format!(
            r#"<div class="z-cal-bar"><nav class="z-cal-nav" aria-label="{}" data-zeyn-drives="{}">"#,
            t("navigation"),
            escape(self.topics[0])
        ));
        out.push_str(&self.link(r, self.view, self.step(day, false), &t("previous"), false));
        out.push_str(&self.link(r, self.view, today, &t("today"), false));
        out.push_str(&self.link(r, self.view, self.step(day, true), &t("next"), false));
        out.push_str(&format!(
            r#"</nav><h2 class="z-cal-period" id="{id}-period" aria-live="polite">{}</h2><nav class="z-cal-views" aria-label="{}" data-zeyn-drives="{}">"#,
            self.period(r, day, first, last),
            t("views"),
            escape(self.topics[0])
        ));
        for v in [CalendarView::Month, CalendarView::Week, CalendarView::Day] {
            out.push_str(&self.link(r, v, day, &t(v.name()), v == self.view));
        }
        out.push_str("</nav></div>");
    }

    /// The accessible name of a day: weekday and date, and "today".
    fn day_label(r: &Render, d: Date, today: Date) -> String {
        let mut s = format!("{} {}", Self::day_name(r, d), escape(&d.format(r.locale)));
        if d == today {
            s.push_str(", ");
            s.push_str(&r.text(&Text::key("zeyn.ui.calendar.today")));
        }
        s
    }

    fn month(
        &self,
        out: &mut String,
        r: &Render,
        (day, first, _): (Date, Date, Date),
        events: &[Placed],
    ) {
        let today = Self::today(r);
        let id = dom_id(r, "cal");
        out.push_str(&format!(
            r#"<table class="z-cal-grid z-cal-month" role="grid" aria-labelledby="{id}-period" aria-readonly="true"><thead><tr>"#
        ));
        for i in 0..7 {
            let d = first.add_days(i).unwrap_or(first);
            out.push_str(&format!(
                r#"<th scope="col" role="columnheader">{}</th>"#,
                Self::day_name(r, d)
            ));
        }
        out.push_str("</tr></thead><tbody>");
        for w in 0..6 {
            out.push_str("<tr>");
            for i in 0..7 {
                let d = first
                    .add_days(w * 7 + i)
                    .unwrap_or(first);
                let here: Vec<&Placed> = events
                    .iter()
                    .filter(|p| p.start.date <= d && Self::last_day(p) >= d)
                    .collect();
                let mut class = String::from("z-cal-day");
                if d.month() != day.month() {
                    class.push_str(" z-cal-out");
                }
                out.push_str(&format!(
                    r#"<td class="{class}" role="gridcell" data-day="{d}" tabindex="{}"{}><span class="z-cal-num" aria-hidden="true">{}</span><span class="z-sr-only">{}</span>"#,
                    if d == day { 0 } else { -1 },
                    if d == today { r#" aria-current="date""# } else { "" },
                    d.day(),
                    Self::day_label(r, d, today)
                ));
                if !here.is_empty() {
                    out.push_str(r#"<div class="z-cal-events">"#);
                    for p in here.iter().take(MONTH_CELL_EVENTS) {
                        out.push_str(&p.html);
                    }
                    out.push_str("</div>");
                    if here.len() > MONTH_CELL_EVENTS {
                        out.push_str(&format!(
                            r#"<span class="z-cal-more">{}</span>"#,
                            r.text(
                                &Text::key("zeyn.ui.calendar.more")
                                    .arg("n", (here.len() - MONTH_CELL_EVENTS).to_string())
                            )
                        ));
                    }
                }
                out.push_str("</td>");
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table>");
    }

    fn hours_view(
        &self,
        out: &mut String,
        r: &Render,
        (_, first, last): (Date, Date, Date),
        events: &[Placed],
    ) {
        let today = Self::today(r);
        let id = dom_id(r, "cal");
        let days: Vec<Date> = (0..=(last.days_since_epoch() - first.days_since_epoch()))
            .filter_map(|i| first.add_days(i))
            .collect();
        let class = if days.len() == 1 {
            "z-cal-day-view"
        } else {
            "z-cal-week"
        };
        out.push_str(&format!(
            r#"<table class="z-cal-grid {class}" role="grid" aria-labelledby="{id}-period" aria-readonly="true"><thead><tr><td class="z-cal-corner"></td>"#
        ));
        for d in &days {
            out.push_str(&format!(
                r#"<th scope="col" role="columnheader"{}><span class="z-cal-wd">{}</span> <span class="z-cal-num">{}</span></th>"#,
                if *d == today { r#" aria-current="date""# } else { "" },
                Self::day_name(r, *d),
                d.day()
            ));
        }
        out.push_str("</tr></thead><tbody>");
        let long = |p: &Placed| Self::last_day(p) > p.start.date;
        out.push_str(&format!(
            r#"<tr class="z-cal-allday"><th scope="row" role="rowheader">{}</th>"#,
            r.text(&Text::key("zeyn.ui.calendar.all_day"))
        ));
        let selected = self.date.unwrap_or(today);
        for d in &days {
            out.push_str(&format!(
                r#"<td role="gridcell" class="z-cal-slot" data-day="{d}" tabindex="-1"><span class="z-sr-only">{}</span>"#,
                Self::day_label(r, *d, today)
            ));
            for p in events
                .iter()
                .filter(|p| long(p) && p.start.date <= *d && Self::last_day(p) >= *d)
            {
                out.push_str(&p.html);
            }
            out.push_str("</td>");
        }
        out.push_str("</tr>");
        let (from, to) = self.hours;
        let mut focus_set = false;
        for h in from..to {
            let hour = Time::new(h, 0, 0).unwrap_or_default();
            out.push_str(&format!(
                r#"<tr><th scope="row" role="rowheader" class="z-cal-hour">{}</th>"#,
                escape(&hour.format(r.locale))
            ));
            for d in &days {
                let focus = !focus_set && *d == selected.max(first).min(last);
                focus_set |= focus;
                out.push_str(&format!(
                    r#"<td role="gridcell" class="z-cal-slot" data-slot="{}T{:02}:00" tabindex="{}"><span class="z-sr-only">{}, {}</span>"#,
                    d,
                    h,
                    if focus { 0 } else { -1 },
                    Self::day_label(r, *d, today),
                    escape(&hour.format(r.locale))
                ));
                for p in events.iter().filter(|p| {
                    !long(p) && p.start.date == *d && p.start.time.hour().clamp(from, to - 1) == h
                }) {
                    out.push_str(&p.html);
                }
                out.push_str("</td>");
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table>");
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let today = Self::today(r);
        let range = self.range(today, r.locale);
        let events = self.placed(r, page);
        let can_create = self
            .create
            .as_ref()
            .is_some_and(|c| r.can(&c.action().access));
        let texts = json!({
            "failed": r.i18n.text(r.locale, &Text::key("zeyn.ui.load_failed")),
        });
        out.push_str(&format!(
            r#"<section class="z-cal" data-zeyn-calendar data-view="{}"{}{} data-texts="{}" data-testid="{}.calendar">"#,
            self.view.name(),
            topics_attr("data-zeyn-listens", &self.topics),
            if can_create { r#" data-create"# } else { "" },
            attr(&texts),
            r.scope
        ));
        if let Some(title) = &self.title {
            out.push_str(&format!(
                r#"<h2 class="z-cal-title">{}</h2>"#,
                r.text(title)
            ));
        }
        self.toolbar(out, r, range);
        out.push_str(r#"<div class="z-cal-scroll">"#);
        match self.view {
            CalendarView::Month => self.month(out, r, range, &events),
            _ => self.hours_view(out, r, range, &events),
        }
        out.push_str("</div>");
        if page.is_some_and(|p| p.count as u32 >= MAX_CALENDAR_EVENTS) {
            out.push_str(&format!(
                r#"<p class="z-cal-cut" role="note">{}</p>"#,
                r.text(
                    &Text::key("zeyn.ui.calendar.cut").arg("n", MAX_CALENDAR_EVENTS.to_string())
                )
            ));
        }
        out.push_str(&format!(
            r#"<p class="z-sr-only" id="{}-help">{}</p><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-calendar-status></p></section>"#,
            dom_id(r, "cal"),
            r.text(&Text::key(if can_create {
                "zeyn.ui.calendar.help_create"
            } else {
                "zeyn.ui.calendar.help"
            }))
        ));
    }
}

impl<E: Model> GridSource for Calendar<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self, _view: Option<&crate::grid::DataGridView>) -> DataQuery {
        let mut query = DataQuery::default();
        self.allow(
            &mut query,
            DateTime::now()
                .local(TimeZone::UTC)
                .date,
            "en",
        );
        query
    }

    fn clean_view(&self, _view: crate::grid::DataGridView) -> crate::grid::DataGridView {
        crate::grid::DataGridView::default()
    }

    fn head_html(&self, _r: &Render, _view: Option<&crate::grid::DataGridView>) -> String {
        String::new()
    }

    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        query.time_zone = ctx.time_zone();
        self.allow(&mut query, ctx.today(), ctx.locale());
        let fetch = self.fetch;
        Box::pin(async move {
            check_masks::<E>(&ctx)?;
            let page = fetch(ctx, query).await?;
            Ok(LoadedPage {
                count: page.items.len(),
                total: page.total,
                next: None,
                offset: 0,
                totals: Vec::new(),
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        let mut out = String::new();
        self.render_with(&mut out, r, Some(page));
        out
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "description": "Events of the visible period. The server sets the query from the screen's view and date." })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for Calendar<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        json!({
            "type": "calendar",
            "model": E::NAME,
            "action": self.action,
            "view": self.view.name(),
            "date": self.date,
            "create": self.create.as_ref().map(|c| c.action()),
            "topics": self.topics,
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        match key {
            "create" => self.create.as_deref(),
            _ => None,
        }
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
        check.drives(self.topics[0]);
        if self.start.is_none() {
            check.problem("a Calendar needs `.start(field)`, when an event starts");
        }
    }
}

impl<E: Model> From<Calendar<E>> for Node {
    fn from(c: Calendar<E>) -> Self {
        Node::new(c)
    }
}
