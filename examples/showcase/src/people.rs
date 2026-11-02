//! People: a leave request form with dates, an email and rules across fields.

use zeyn::prelude::*;

/// A leave request. Not stored as a table row in this example.
#[model(validate = check_days)]
pub struct LeaveRequest {
    #[rule(required, max_len = 120)]
    pub employee: String,
    #[rule(required, email)]
    pub manager_email: String,
    pub first_day: Date,
    pub last_day: Date,
    pub kind: LeaveKind,
    #[rule(max_len = 500)]
    pub note: Option<String>,
}

/// Kind of leave.
#[choice]
pub enum LeaveKind {
    Annual,
    Sick,
    Unpaid,
}

impl LeaveRequest {
    /// Calendar days covered, both ends included.
    pub fn days(&self) -> i64 {
        self.last_day.days_since_epoch() - self.first_day.days_since_epoch() + 1
    }
}

fn check_days(r: &LeaveRequest, errors: &mut ValidationErrors) {
    if r.last_day < r.first_day {
        errors.add(LeaveRequest::last_day, t!("people.ends_before_start"));
    } else if r.days() > 30 {
        errors.add(LeaveRequest::last_day, t!("people.too_long"));
    }
}

/// Sends the request. A real app would store it and notify the manager.
#[action(permission = "people.leave.request", audit)]
pub async fn send_leave(request: Valid<LeaveRequest>) -> Result<i64> {
    Ok(request.days())
}

#[screen(route = "/leave", permission = "people.leave.request")]
pub fn leave(_: (), ctx: Ctx) -> Screen {
    let today = ctx.today();
    let request = LeaveRequest {
        employee: ctx.identity().user.clone().unwrap_or_default(),
        first_day: today.add_days(7).unwrap_or(today),
        last_day: today.add_days(11).unwrap_or(today),
        ..LeaveRequest::default()
    };
    Screen::new(t!("people.leave")).body(
        Form::of(request)
            .row(|r| r.field(LeaveRequest::employee).field(LeaveRequest::manager_email))
            .row(|r| r.field(LeaveRequest::first_day).field(LeaveRequest::last_day).field(LeaveRequest::kind))
            .field(LeaveRequest::note)
            .submit(t!("people.send"), send_leave)
            .on_success(|days, ui| {
                if let Ok(d) = days {
                    ui.toast(t!("people.sent", days = d));
                }
            }),
    )
}

pub fn feature() -> Feature {
    Feature::new("people")
        .route_prefix("/people")
        .guard(SignedIn)
        .guard(Licensed::module("people"))
        .screen(leave)
        .menu(|m| m.group(t!("menu.people"), Icon::Edit).item(t!("people.leave"), leave))
}
