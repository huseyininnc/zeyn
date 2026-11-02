//! People: leave requests with dates, an email and rules across fields, a typed approval
//! workflow (submit, approve, reject), a masked email and audited reads of sick leave data.

use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
use zeyn::{Mail, Mailer};

/// A leave request. Leave data can reveal health information, so reads are audited.
#[entity(table = "leave_requests", validate = check_days, sensitive)]
pub struct LeaveRequest {
    #[key]
    pub id: LeaveRequestId,
    #[rule(required, max_len = 120)]
    pub employee: String,
    #[rule(required, email)]
    #[mask(email)]
    pub manager_email: String,
    pub first_day: Date,
    pub last_day: Date,
    pub kind: LeaveKind,
    #[rule(max_len = 500)]
    pub note: Option<String>,
    #[readonly]
    pub state: LeaveState,
}

/// Kind of leave.
#[choice]
pub enum LeaveKind {
    Annual,
    Sick,
    Unpaid,
}

/// Where a leave request is in its approval.
#[choice]
pub enum LeaveState {
    Draft,
    Submitted,
    Approved,
    Rejected,
}

/// Who may move a leave request, and from where to where. Checked on the server.
pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
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

/// Leave requests in memory. A real app keeps them in a database.
#[derive(Clone, Default)]
pub struct LeaveStore(Arc<RwLock<Vec<LeaveRequest>>>);

impl Service for LeaveStore {}

impl LeaveStore {
    /// One request waiting for approval, so the workflow can be tried at once.
    pub fn seeded(today: Date) -> Self {
        let first = today.add_days(14).unwrap_or(today);
        let waiting = LeaveRequest {
            id: LeaveRequestId(1),
            employee: "Deniz Aydın".into(),
            manager_email: "selin.kaya@example.com".into(),
            first_day: first,
            last_day: first.add_days(4).unwrap_or(first),
            kind: LeaveKind::Annual,
            note: None,
            state: LeaveState::Submitted,
        };
        LeaveStore(Arc::new(RwLock::new(vec![waiting])))
    }

    fn get(&self, id: LeaveRequestId) -> Result<LeaveRequest> {
        self.0.read().map_err(|_| Error::Internal("leave store lock".into()))?.iter().find(|r| r.id == id).cloned().ok_or(Error::NotFound)
    }
}

/// Saves a new request as a draft, or changes a draft. Submitted requests cannot be changed.
#[action(permission = "people.leave.request", audit)]
pub async fn save_leave(request: Valid<LeaveRequest>, store: LeaveStore, ctx: Ctx) -> Result<LeaveRequestId> {
    let mut rows = store.0.write().map_err(|_| Error::Internal("leave store lock".into()))?;
    let mut saved = request.into_inner();
    if saved.id.0 == 0 {
        saved.id = LeaveRequestId(rows.iter().map(|r| r.id.0).max().unwrap_or(0) + 1);
        saved.state = LeaveState::Draft;
        rows.push(saved.clone());
        return Ok(saved.id);
    }
    let current = rows.iter_mut().find(|r| r.id == saved.id).ok_or(Error::NotFound)?;
    if current.state != LeaveState::Draft {
        return Err(Error::Conflict);
    }
    ctx.record_changes(&*current, &saved);
    saved.state = current.state;
    *current = saved.clone();
    Ok(saved.id)
}

/// Moves a request through its workflow and tells the manager by e-mail. The mail is only
/// queued here; a background job sends it.
#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(request: TransitionRequest<LeaveRequestId>, store: LeaveStore, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let (state, manager, employee) = {
        let mut rows = store.0.write().map_err(|_| Error::Internal("leave store lock".into()))?;
        let row = rows.iter_mut().find(|r| r.id == request.id).ok_or(Error::NotFound)?;
        row.state = leave_flow().apply(&ctx, row.state, &request.transition, request.comment.as_deref())?;
        (row.state, row.manager_email.clone(), row.employee.clone())
    };
    mailer.send(&ctx, Mail::transition(&ctx, &manager, &employee)?)?;
    Ok(state)
}

/// Imports leave requests from a CSV file as drafts.
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> {
    let mut all = store.0.write().map_err(|_| Error::Internal("leave store lock".into()))?;
    let count = rows.len();
    for row in rows {
        let mut r = row.into_inner();
        r.id = LeaveRequestId(all.iter().map(|x| x.id.0).max().unwrap_or(0) + 1);
        r.state = LeaveState::Draft;
        all.push(r);
    }
    Ok(count)
}

#[screen(route = "/import", permission = "people.leave.import")]
pub fn leave_import() -> Screen {
    Screen::new(t!("people.import")).body(Import::of(import_leaves).max_rows(1000))
}

#[action(permission = "people.leave.request")]
pub async fn search_leaves(q: DataQuery, store: LeaveStore) -> Result<Page<LeaveRequest>> {
    let rows = store.0.read().map_err(|_| Error::Internal("leave store lock".into()))?.clone();
    Ok(q.apply(rows))
}

#[params]
pub struct LeaveParams {
    pub id: i64,
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
            .submit(t!("people.send"), save_leave)
            .on_success(|saved, ui| {
                if saved.is_ok() {
                    ui.toast(t!("people.saved_draft"));
                    ui.navigate(leaves, ());
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
pub async fn search_employees(q: DataQuery) -> Result<Page<Employee>> {
    Ok(q.apply(employees()))
}

/// The organization chart as a tree: open a person to see who reports to them.
#[screen(route = "/org", permission = "people.org.read")]
pub fn org_chart() -> Screen {
    Screen::new(t!("people.org")).body(
        DataGrid::of(search_employees)
            .mode(DataMode::Server)
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

#[screen(route = "/leaves", permission = "people.leave.request")]
pub fn leaves() -> Screen {
    Screen::new(t!("people.requests")).body(
        DataGrid::of(search_leaves)
            .column(LeaveRequest::employee)
            .filter()
            .column(LeaveRequest::manager_email)
            .column(LeaveRequest::first_day)
            .column(LeaveRequest::last_day)
            .column(LeaveRequest::kind)
            .column(LeaveRequest::state)
            .on_open(|r| Window::open(leave_view, LeaveParams { id: r.id.0 })),
    )
}

#[screen(permission = "people.leave.request")]
pub fn leave_view(params: LeaveParams, store: LeaveStore) -> Result<Screen> {
    let request = store.get(LeaveRequestId(params.id))?;
    let form = Form::of(request.clone())
        .row(|r| r.field(LeaveRequest::employee).field(LeaveRequest::manager_email))
        .row(|r| r.field(LeaveRequest::first_day).field(LeaveRequest::last_day).field(LeaveRequest::kind))
        .field(LeaveRequest::note);
    let details: Node = if request.state == LeaveState::Draft {
        form.submit(t!("people.save"), save_leave).on_success(|_, ui| ui.close_and_refresh()).into()
    } else {
        form.into()
    };
    Ok(Screen::window(t!("people.leave")).width(640).body(details).body(
        Transitions::of(leave_flow(), request.state, request.id, move_leave).on_success(|_, ui| ui.close_and_refresh()),
    ))
}

pub fn feature() -> Feature {
    Feature::new("people")
        .route_prefix("/people")
        .guard(SignedIn)
        .guard(Licensed::module("people"))
        .screen(leave)
        .screen(org_chart)
        .screen(leaves)
        .screen(leave_view)
        .screen(leave_import)
        .menu(|m| {
            m.group(t!("menu.people"), Icon::Edit)
                .item(t!("people.leave"), leave)
                .item(t!("people.requests"), leaves)
                .item(t!("people.import"), leave_import)
                .item(t!("people.org"), org_chart)
        })
}
