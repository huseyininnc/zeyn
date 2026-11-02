# Enterprise Features

Cross cutting features for business apps: data audit, personal data protection, safe deploys,
health checks, session safety, CSV import, background jobs and approval workflows. Everything is
checked on the server; the browser runtime only makes it pleasant to use. KVKK and GDPR are the
privacy laws kept in mind, but nothing here is specific to a country or an industry.

## Field level data audit

`AuditLog` writes one `AuditEntry` per form submit and per call to an action marked `audit`. For
saves the entry also names the record and every field that changed:

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- Forms do this on their own: the old values come from the record the screen loaded on the
  server, the new values from the validated `Valid<M>`. Nothing the client sends is used as an
  old value, and fields the form does not take from the client never show up as changed.
- API saves record the model and key from their `Valid<M>` input. Only the action knows what is in
  the database, so it reports the difference itself:

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  Doing the same in form save actions is fine: the action's report replaces the form's.
- Values are plain, language independent strings (`1234.56 TRY`, `2024-05-01`, choice values).
  Row versions are left out. Masked fields stay masked (see below).
- A failed save still names the record, with no changes.

## Field masking

Mark text fields that hold personal data with `#[mask]`. Here the national id shows as `•••• 8901`, the email as `p••••@example.com` and the notes as `••••`:

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- Masking happens on the server, in grid cells, forms, API output, agent tools and therefore in
  CSV exports, which are built from the cells. The raw value never reaches a browser or an API
  client without the permission.
- The unmask permission is `<model>.<field>.unmask` in snake case (`person.national_id.unmask`)
  unless the attribute names one.
- Users with the permission still see the masked text first, with a reveal button. Revealing calls
  `POST /_zeyn/reveal`, which checks the permission again and writes an audit record (kind
  `reveal`, field in `revealed`). Refused attempts are audited too.
- API clients with the permission get raw values; each such field is listed in `revealed` of the
  call's audit record.
- Forms show masked fields as read only text. The stored value is kept on save unless the user
  may reveal it and changed it after revealing. An empty masked field is a normal input, so data
  entry staff can fill it in once without being able to read it back later.
- Grid filters and sorts on a masked field are ignored for users who may not reveal it, so a value
  cannot be guessed by searching. Do not add `.filter()` to masked columns.
- Grid reveal looks the row up through the grid's action with a filter on the key, so the action
  must honor filters on its key field.
- `#[mask]` only compiles on `String` and `Option<String>`.

## Sensitive record reads

Who saw which personal data, and when, is recorded as well as who changed it.

- `#[entity(table = "cases", sensitive)]` (or `#[sensitive]` below `#[entity]`) marks a model.
  Every time a user is shown such a record, the call's audit record lists it in `reads`
  (`{"model":"Case","id":"2"}`): rows of a grid page, a form, an agent tool result and API
  output. Reads are recorded where the data is rendered, so components inside layouts count too.
- `#[screen(..., sensitive)]` records every opening of the screen as a read whose `model` is the
  screen name and `id` its params, for screens that show personal data not tied to one model.
- `#[screen(..., reason)]` also asks the user why before any data is shown. The server answers
  `428` with code `reason_required` (a page with a reason form on full page loads) to the page,
  its fragments, grid, form and reveal calls until a reason is given. The runtime asks in a
  dialog, sends `POST /_zeyn/reason {screen, reason}` (3 to 300 characters) and repeats the
  request. The reason is kept for 15 minutes in an `HttpOnly`, `SameSite=Strict` cookie, only
  opens the screen it was given for and is written in `reason` of every audit record of that
  screen. Agents and API clients cannot give a reason, so they cannot open such screens.
- `ctx.record_read(&record)` records a read from application code.
- Read records are written by `AuditLog` for any call, page loads included, as soon as a sensitive
  record was shown. Without an audit interceptor nothing is recorded; apps that hold personal
  data should always register one.

## Approval workflows

A workflow is a small, typed state machine. States are a `#[choice]` enum; each move names its
start and end state and the permission it needs:

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply` runs on the server with the state read from the store. An unknown move is `404`, a
  record that is no longer in the start state is `409` (someone moved it first), a missing
  permission is `403`, a comment over 1000 characters is `422`.
- Each move is written to the audit record in `transition`: name, from, to and comment; the user
  and time are in the record itself.
- `Transitions::of(flow, state, id, action)` shows the current state, an optional comment and one
  button per move the user may make from it (`Workflow::allowed`). The record key comes from the
  screen on the server, so a client cannot move another record. Agents see it as a form tool.
- Button labels come from `workflow.<name>` in the catalogs; `.label(...)` sets one by hand.
- `Workflow::check` reports invalid permissions and moves declared twice from the same state;
  call it in a test.
- The showcase leave requests use this: save as draft, submit, then approve or reject.

## Safe deploys

- Every response carries `x-zeyn-build`, and every page `<meta name="zeyn-build">`. The id is
  `ZEYN_BUILD_ID` when set (letters, digits, `.`, `_`, `-`, up to 64), otherwise a hash of the
  executable. `zeyn::server::build_id()` returns it, for logs.
- When the runtime sees a response from another build, it shows a polite notice: a new version is
  available, reload when ready. It never reloads by itself.
- What users type in forms is kept in `sessionStorage` per screen, params and form, and put back
  after a reload with a note and a button to discard it. Masked, password, file and hidden inputs
  are never kept. The draft keeps the row version it was typed on, so saving a draft of a record
  someone changed in the meantime is refused as a conflict. A successful save clears the draft;
  drafts older than a day are dropped. `sessionStorage` lives only in that browser tab.

## Health checks

- `GET /_zeyn/health` answers `200 ok` while the process runs. Use it for liveness.
- `GET /_zeyn/ready` runs every check added with `App::readiness(name, check)` at the same time,
  five seconds each, and answers `200 {"status":"ready"}` or
  `503 {"status":"not_ready","failed":["database"]}`. Only check names are shown, never errors.
- Both live under `base_path` (`/billing/_zeyn/health`), answer with `Cache-Control: no-store`
  and need no sign in.
- They are never rate limited, so a busy probe cannot lock itself out and restart a healthy app.
- IP rules still apply: `deny_ips` always, `allow_only_ips` too unless the probe address is listed
  in `App::probe_ips([...])`, which opens only these two endpoints to it.

## Development latency

`App::dev_latency(Duration::from_millis(400))` delays every call (pages, fragments, grids, forms,
API, MCP and runtime calls; not static assets or health probes) so developers see loading states
and double clicks the way users on a slow network do. It works only in debug builds: in release
builds the method does nothing and the delay code is compiled out, so leaving it in by mistake
cannot slow production. A warning is printed at startup while it is on.

## Session safety

Sign in is external (a single sign on proxy today, OIDC later), so these features only need the
authenticator to say who the user is and, for re authentication, when they signed in
(`Identity::authenticated_at`; `ForwardAuth::auth_time("x-auth-time")` reads Unix seconds or
RFC 3339; `DevAuth` reports every request as freshly signed in).

- `App::idle_timeout(Duration)` makes the runtime warn the user `idle_warning` before the end
  (one minute by default) in an alert dialog with a countdown and a "stay signed in" button, which
  calls `POST /_zeyn/keepalive` so a proxy in front sees activity. Input in any tab of the app
  counts. At the end the user goes to `logout_url`, else to `login_url` with `return_to`, else
  sees a signed out screen. Form drafts are kept. The idle timeout only closes forgotten screens;
  ending the real session is the identity provider's job.
- `#[action(permission = "...", reauth)]` needs a sign in within `App::reauth_within` (five
  minutes by default). Otherwise the server answers `401` with code `reauth_required` (problem
  type `urn:zeyn:problem:reauth_required` on the API), whatever the client sends. The runtime asks
  the user to sign in again in a new tab (`reauth_url`, else `login_url`) and repeats the request
  when they come back, so nothing typed is lost. Identities without a sign in time never pass.

## CSV import

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- The user picks a CSV file. The runtime reads it as UTF-8, or as Windows-1254 when it is not
  valid UTF-8 (Turkish spreadsheets), and sends the text in a JSON body to `POST /_zeyn/import`
  (no multipart needed; at most 8 MB). The server answers with a summary, a preview of the first
  ten rows, the errors per row and a downloadable error report.
- Parsing: byte order mark dropped, `,`, `;` or tab detected from the header line, `"` quoting with
  separators and line breaks inside, `\r\n` or `\n`.
- Columns are matched by field name or label in any language of the app, ignoring case and Turkish
  dotted and dotless i. Unknown, read only, key and row version columns are refused.
- Every row is read and validated exactly like a form of the model: values as a user of that
  language types them (`1.234,56`, `05.12.2026`), the field rules and the model's own `validate`.
- Import sends only the valid rows to the action; `.all_or_nothing()` imports nothing unless
  every row is valid. On import the server reads and validates the file again; nothing from the
  preview is trusted. Both steps need the action's permission, and the import is audited.
- `.max_rows(n)` limits rows (5000 by default, never more than 50000). The error report defuses
  spreadsheet formulas. Masked fields are shown masked in the preview.

## Background jobs

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- A job is `async fn(Ctx) -> Result<()>`. Its context has the app's services, configuration and
  default language, and the identity `job:<name>` with no permissions.
- `Schedule::every(d)` runs every interval; `Schedule::manual()` only when triggered. Actions
  trigger jobs through the injectable `Jobs` service: `jobs.trigger("rebuild_index")`. A job
  triggered while it runs runs once more afterwards; a job never runs twice at the same time.
- `.retry(times, first_delay)` tries a failed run again, doubling the wait up to ten minutes.
- At most `job_concurrency` jobs (four by default) run at once.
- Status (state, runs, failures, last start and end, last error code) is `jobs.status()` in code
  and `GET /_zeyn/jobs` for users with `jobs_permission`. Error details only go to the server log.
- `App::serve_until(addr, signal)` stops taking requests when the signal completes, lets running
  requests finish, cancels retry waits and gives running jobs 30 seconds.
- Jobs are in memory. Work that is waiting or running when the process stops is lost, and with
  several servers each runs its own schedule. Durable jobs that survive restarts and run once per
  cluster need a database backed queue, which will come with the data layer.
