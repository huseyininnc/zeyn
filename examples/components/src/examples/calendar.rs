//! Calendar examples: a month, week or day of events in the visitor's time zone, with an event
//! window and creating by choosing a slot.

use crate::{Demo, Entry, exd};
use zeyn::prelude::*;
use zeyn::ui::{CalendarParams, CalendarSlot};

/// An event of the sample calendar.
#[entity(table = "events")]
pub struct Appointment {
    #[key]
    pub id: AppointmentId,
    pub title: String,
    pub starts: DateTime,
    pub ends: DateTime,
}

/// The sample events around `today`, at local hours of the visitor's zone.
fn sample(today: Date, zone: TimeZone) -> Vec<Appointment> {
    let plan = [
        (-3, 9, 1, "Supplier call"),
        (-1, 14, 2, "Stock count"),
        (0, 10, 1, "Team stand up"),
        (0, 15, 2, "Design review"),
        (1, 11, 1, "Customer visit"),
        (2, 9, 8, "Trade fair"),
        (6, 13, 1, "Payroll run"),
        (9, 10, 3, "Workshop"),
    ];
    plan.iter()
        .enumerate()
        .filter_map(|(i, (day, hour, hours, title))| {
            let date = today.add_days(*day)?;
            let at = LocalDateTime::new(date, Time::new(*hour, 0, 0)?);
            let starts = DateTime::from_local(at, zone).ok()?;
            Some(Appointment {
                id: AppointmentId(i as i64 + 1),
                title: title.to_string(),
                starts,
                ends: starts.add_ms(*hours * 3_600_000),
            })
        })
        .collect()
}

/// The sample events in the visible range the calendar asks for.
#[action(public)]
pub async fn sample_appointments(q: DataQuery, ctx: Ctx) -> Result<Page<Appointment>> {
    Ok(q.apply(sample(ctx.today(), ctx.time_zone())))
}

/// Shows the chosen slot; a real app opens a form window for the new event here. The catalog
/// stores nothing a visitor creates, so its events are always the samples.
#[action(public)]
pub async fn new_appointment(slot: CalendarSlot, ctx: Ctx) -> Result<Option<Effect>> {
    let zone = ctx.time_zone();
    Ok(Some(Effect::Toast {
        message: Text::from(format!(
            "New event from {} to {}",
            slot.start.format(zone, ctx.locale()),
            slot.end.format(zone, ctx.locale())
        )),
    }))
}

/// The window of one sample event.
#[screen(public)]
pub fn appointment(p: AppointmentParams, ctx: Ctx) -> Screen {
    let found = sample(ctx.today(), ctx.time_zone())
        .into_iter()
        .find(|a| a.id.0 == p.id);
    let text = found
        .map(|a| {
            format!(
                "{}: {}",
                a.title,
                a.starts
                    .format(ctx.time_zone(), ctx.locale())
            )
        })
        .unwrap_or_default();
    Screen::window("Event").body(Paragraph::new(text))
}

/// Which sample event a window shows.
#[params]
pub struct AppointmentParams {
    pub id: i64,
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    Calendar::of(sample_appointments)
        .start(Appointment::starts)
        .end(Appointment::ends)
        .label(|a: &Appointment| a.title.clone())
        .open(|a: &Appointment| Window::open(appointment, AppointmentParams { id: a.id.0 }))
        .create(new_appointment)
        .at(CalendarParams {
            view: demo.view,
            date: demo.date,
        })
        .into()
}

/// What the catalog shows of `Calendar`.
pub const ENTRY: Entry = Entry {
    name: "Calendar",
    docs: "components/admin#calendar",
    api: &["Calendar"],
    overview: &[exd!(overview)],
    examples: &[],
};
