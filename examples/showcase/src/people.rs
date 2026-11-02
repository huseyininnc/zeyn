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

/// A person in the organization chart.
#[entity(table = "employees")]
pub struct Employee {
    #[key]
    pub id: EmployeeId,
    pub name: String,
    pub title: String,
    pub manager_id: Option<EmployeeId>,
    pub salary: Money,
}

const FIRST: [&str; 12] = ["Ada", "Can", "Elif", "Mert", "Zeynep", "Ömer", "İpek", "Deniz", "Selin", "Kaan", "Derya", "Emre"];
const LAST: [&str; 9] = ["Yılmaz", "Demir", "Şahin", "Kaya", "Arslan", "Çelik", "Aydın", "Koç", "Öztürk"];

/// The organization: one chief, five directors, four managers under each and six people under
/// every manager, 146 people in all.
pub fn employees() -> Vec<Employee> {
    let mut all = Vec::new();
    let mut add = |id: i64, manager: Option<i64>, title: &str, salary: i64| {
        all.push(Employee {
            id: EmployeeId(id),
            name: format!("{} {}", FIRST[(id % 12) as usize], LAST[(id * 7 % 9) as usize]),
            title: title.to_string(),
            manager_id: manager.map(EmployeeId),
            salary: Money::tl(salary + id * 1_300),
        })
    };
    add(1, None, "Chief executive", 9_000_000);
    let mut next = 2;
    for _ in 0..5 {
        let director = next;
        add(director, Some(1), "Director", 5_200_000);
        next += 1;
        for _ in 0..4 {
            let manager = next;
            add(manager, Some(director), "Manager", 3_400_000);
            next += 1;
            for _ in 0..6 {
                add(next, Some(manager), "Specialist", 2_100_000);
                next += 1;
            }
        }
    }
    all
}

#[action(permission = "people.org.read")]
pub async fn search_employees(q: GridQuery) -> Result<Page<Employee>> {
    Ok(q.apply(employees()))
}

/// The organization chart as a tree: open a person to see who reports to them.
#[screen(route = "/org", permission = "people.org.read")]
pub fn org_chart() -> Screen {
    Screen::new(t!("people.org")).body(
        Grid::of(search_employees)
            .mode(GridMode::Server)
            .paging(Paging::pages(25))
            .tree(Employee::manager_id)
            .column(Employee::name)
            .flex(2)
            .filter()
            .column(Employee::title)
            .flex(1)
            .filter()
            .column(Employee::salary)
            .width(240)
            .sum()
            .avg(),
    )
}

pub fn feature() -> Feature {
    Feature::new("people")
        .route_prefix("/people")
        .guard(SignedIn)
        .guard(Licensed::module("people"))
        .screen(leave)
        .screen(org_chart)
        .menu(|m| m.group(t!("menu.people"), Icon::Edit).item(t!("people.leave"), leave).item(t!("people.org"), org_chart))
}
