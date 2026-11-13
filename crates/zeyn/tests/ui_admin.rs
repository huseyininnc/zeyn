//! Admin components: DiffView, CodeView and JsonView, barcodes, the inbox and the notification
//! center, Kanban boards and calendars.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    Barcode, CalendarParams, CalendarSlot, CodeLang, CodeView, DiffLayout, DiffView, Inbox,
    InboxSelection, JsonView, ListItem, MAX_CODE_LINES, NotificationCenter, QrCode, QrLevel,
    QrMatrix, UnreadCount, code128_symbols, highlight, qr_error_correction, qr_format_bits,
    qr_version_bits,
};
use zeyn::{Component, FieldChange, I18n, Identity, Render};

#[choice]
pub enum Stage {
    Draft,
    Approved,
}

#[model]
pub struct Account {
    pub name: String,
    #[mask(last4)]
    pub iban: String,
    pub stage: Stage,
}

fn html(c: &impl Component) -> String {
    let i18n = I18n::builtin();
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", "en", i18n));
    out
}

fn account(iban: &str, stage: Stage) -> Account {
    Account {
        name: "Ada".into(),
        iban: iban.into(),
        stage,
    }
}

#[test]
fn a_diff_shows_only_changed_fields_side_by_side_with_labels_and_choice_labels() {
    let old = account("TR12 3456 7890 1111", Stage::Draft);
    let new = account("TR12 3456 7890 2222", Stage::Approved);
    let page = html(&DiffView::of(&old, &new).title("History"));
    assert!(page.contains(r#"<table class="z-diff-table" aria-labelledby="s-diff--title">"#));
    assert!(page.contains(r#"<th scope="col">Before</th><th scope="col">After</th>"#));
    assert!(page.contains("<del>Draft</del>") && page.contains("<ins>Approved</ins>"));
    assert!(!page.contains(">Ada<"), "unchanged fields are left out");
}

#[test]
fn a_diff_never_shows_a_masked_value_even_from_a_hand_built_change() {
    let old = account("TR12 3456 7890 1111", Stage::Draft);
    let new = account("TR12 3456 7890 2222", Stage::Draft);
    let page = html(&DiffView::of(&old, &new));
    assert!(page.contains("•••• 1111") && page.contains("•••• 2222"));
    assert!(!page.contains("3456"), "{page}");
    let raw = DiffView::changes([FieldChange {
        field: "iban",
        old: Some("TR12 3456 7890 1111".into()),
        new: None,
    }])
    .model::<Account>()
    .layout(DiffLayout::Inline);
    let page = html(&raw);
    assert!(!page.contains("3456"), "{page}");
    assert!(page.contains(r#"<dl class="z-diff-list" aria-label="Changes">"#));
    assert!(page.contains(r#"<span class="z-diff-empty">(empty)</span>"#));
    assert!(
        !raw.schema()
            .to_string()
            .contains("7890")
    );
}

#[test]
fn a_diff_without_changes_says_so_and_escapes_values() {
    assert!(html(&DiffView::changes([])).contains("No changes"));
    let page = html(&DiffView::changes([FieldChange {
        field: "note",
        old: Some("<b>".into()),
        new: Some("x".into()),
    }]));
    assert!(page.contains("<del>&lt;b&gt;</del>") && page.contains(">note<"));
}

#[test]
fn code_is_highlighted_numbered_and_folded_without_script() {
    let src =
        "fn main() {\n    let s = \"<x>\";\n    if s.is_empty() {\n        run();\n    }\n}\n";
    let page = html(&CodeView::new(CodeLang::Rust, src).title("main.rs"));
    assert!(page.contains(r#"<span class="z-hl-kw">fn</span> main() {"#));
    assert!(page.contains("&quot;&lt;x&gt;&quot;") && !page.contains("<x>"));
    assert_eq!(
        page.matches(r#"<details class="z-codeview-fold" open>"#)
            .count(),
        2
    );
    assert_eq!(page.matches("</details>").count(), 2);
    assert_eq!(
        page.matches(r#"<code class="z-codeview-l">"#)
            .count(),
        6
    );
    assert!(
        page.contains(r#"data-zeyn-copy="s-code--body""#) && page.contains("hidden>Copy</button>")
    );
    assert!(page.contains(r#"role="region" tabindex="0" aria-labelledby="s-code--title""#));
    assert!(page.contains("6 lines") && !page.contains("z-codeview-truncated"));
}

#[test]
fn a_block_comment_over_lines_is_closed_and_opened_on_each_line() {
    let page = html(&CodeView::new(
        CodeLang::Sql,
        "/* a\nb */ SELECT 'it''s' FROM t",
    ));
    assert!(
        page.contains(r#"<span class="z-hl-com">/* a</span></code>"#),
        "{page}"
    );
    assert!(page.contains(r#"<code class="z-codeview-l"><span class="z-hl-com">b */</span> <span class="z-hl-kw">SELECT</span> <span class="z-hl-str">&#39;it&#39;&#39;s&#39;</span>"#), "{page}");
}

#[test]
fn long_text_is_cut_with_a_notice_and_json_is_folded_per_object() {
    let text: String = (0..MAX_CODE_LINES + 5)
        .map(|i| format!("line {i}\n"))
        .collect();
    let view = CodeView::new(CodeLang::Plain, text);
    assert!(view.truncated());
    let page = html(&view);
    assert!(page.contains(&format!(
        "Showing the first {MAX_CODE_LINES} of {} lines",
        MAX_CODE_LINES + 5
    )));
    let json = html(&JsonView::new(
        &serde_json::json!({ "a": [1, 2], "b": { "c": true } }),
    ));
    assert_eq!(json.matches("<details").count(), 3);
    assert!(json.contains(r#"<span class="z-hl-kw">true</span>"#));
}

#[test]
fn the_highlighter_knows_rust_json_toml_and_sql() {
    assert!(
        highlight(CodeLang::Toml, "[dependencies]\nzeyn = 1 # x")
            .starts_with(r#"<span class="z-hl-kw">[dependencies]</span>"#)
    );
    assert!(
        highlight(CodeLang::Json, r#"{"k": "v"}"#)
            .contains(r#"<span class="z-hl-key">&quot;k&quot;</span>"#)
    );
    assert!(
        highlight(CodeLang::Sql, "select \"Name\" from t")
            .contains(r#"<span class="z-hl-key">&quot;Name&quot;</span>"#)
    );
    assert!(
        highlight(CodeLang::Rust, "println!(\"x\")")
            .starts_with(r#"<span class="z-hl-mac">println!</span>"#)
    );
}

#[test]
fn code_128_packs_digit_runs_and_adds_the_check_symbol() {
    assert_eq!(
        code128_symbols("AB1234567"),
        Some(vec![104, 33, 34, 99, 12, 34, 56, 100, 23, 86, 106])
    );
    assert_eq!(code128_symbols("12"), Some(vec![105, 12, 14, 106]));
    assert_eq!(code128_symbols("1"), Some(vec![104, 17, 18, 106]));
    assert!(code128_symbols("").is_none() && code128_symbols("tab	").is_none());
    assert!(code128_symbols(&"9".repeat(81)).is_none());
}

#[test]
fn a_barcode_has_its_quiet_zones_and_a_text_label() {
    let page = html(&Barcode::code128("PJJ123C"));
    let modules = 11 * 9 + 13;
    assert!(
        page.contains(&format!(r#"viewBox="0 0 {} 1""#, modules + 20)),
        "{page}"
    );
    assert!(
        page.contains(r#"d="M10 0h2v1h-2z"#),
        "the first bar starts after 10 quiet modules"
    );
    assert!(page.contains(r#"role="img" aria-labelledby="s-barcode-""#));
    assert!(
        page.contains(r#"<figcaption class="z-barcode-text" id="s-barcode-">PJJ123C</figcaption>"#)
    );
    assert!(html(&Barcode::code128("Ünlü")).contains("z-barcode-error"));
    let labelled = html(&Barcode::code128("4006381333931").label("Tea, 500 g"));
    assert!(labelled.contains(">Tea, 500 g</figcaption>"));
}

#[test]
fn qr_error_correction_format_and_version_bits_match_the_standard() {
    let data = [
        32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17,
    ];
    assert_eq!(
        qr_error_correction(&data, 10),
        [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]
    );
    assert_eq!(qr_format_bits(QrLevel::Low, 0), 0b111011111000100);
    assert_eq!(qr_format_bits(QrLevel::Quartile, 0), 0b011010101011111);
    assert_eq!(qr_format_bits(QrLevel::High, 0), 0b001011010001001);
    assert_eq!(qr_version_bits(7), 0x07C94);
    assert_eq!(qr_version_bits(40), 0x28C69);
}

#[test]
fn a_qr_code_holds_its_data_in_byte_mode_with_finders_and_the_dark_module() {
    let m = QrMatrix::encode(b"HELLO", QrLevel::Medium).unwrap();
    assert_eq!((m.version(), m.size()), (1, 21));
    let words = m.read_codewords();
    assert_eq!(words.len(), 26);
    assert_eq!(&words[..3], &[0x40, 0x54, 0x84]);
    assert_eq!(words[16..], qr_error_correction(&words[..16], 10)[..]);
    for (x, y) in [(0, 0), (6, 0), (0, 6), (20, 0), (0, 20), (8, 13)] {
        assert!(m.dark(x, y), "({x}, {y})");
    }
    assert!(!m.dark(7, 7) && !m.dark(1, 1));
    let long = QrMatrix::encode("x".repeat(300).as_bytes(), QrLevel::Medium).unwrap();
    assert!(long.version() >= 7 && long.size() == long.version() * 4 + 17);
    assert!(QrMatrix::encode(&[b'a'; 2954], QrLevel::Low).is_none());
}

#[test]
fn a_qr_code_renders_with_four_quiet_modules_and_a_label() {
    let page = html(&QrCode::new("https://example.com/o/42").label("Order 42"));
    let m = QrMatrix::encode(b"https://example.com/o/42", QrLevel::Medium).unwrap();
    let side = m.size() + 8;
    assert!(page.contains(&format!(r#"viewBox="0 0 {side} {side}""#)));
    assert!(page.contains(r#"d="M4 4h7v1h-7z"#), "{page}");
    assert!(page.contains(">Order 42</figcaption>"));
    assert!(html(&QrCode::new("x".repeat(3000))).contains("z-barcode-error"));
}

#[entity(table = "notes")]
pub struct Note {
    #[key]
    pub id: NoteId,
    pub title: String,
    pub created_at: DateTime,
    pub read: bool,
}

#[derive(Clone, Default)]
pub struct Notes(Arc<Mutex<Vec<Note>>>);

impl Service for Notes {}

impl Notes {
    fn seeded() -> Notes {
        let now = DateTime::now();
        let day = 86_400_000;
        let note = |id: i64, title: &str, ago: i64, read: bool| Note {
            id: NoteId(id),
            title: title.into(),
            created_at: now.add_ms(-ago),
            read,
        };
        Notes(Arc::new(Mutex::new(vec![
            note(1, "Invoice <paid>", 60_000, false),
            note(2, "Stock low", day, true),
            note(3, "Report ready", 5 * day, false),
        ])))
    }
}

#[action(permission = "notes.read")]
pub async fn search_notes(q: DataQuery, notes: Notes) -> Result<Page<Note>> {
    Ok(q.apply(notes.0.lock().unwrap().clone()))
}

#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, notes: Notes) -> Result<()> {
    for n in notes.0.lock().unwrap().iter_mut() {
        if s.ids.contains(&n.id.0.to_string()) {
            n.read = true;
        }
    }
    Ok(())
}

#[action(permission = "notes.write")]
pub async fn archive_notes(s: InboxSelection, notes: Notes) -> Result<()> {
    notes
        .0
        .lock()
        .unwrap()
        .retain(|n| !s.ids.contains(&n.id.0.to_string()));
    Ok(())
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, notes: Notes) -> Result<u32> {
    let n = notes
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|n| !n.read)
        .take(q.limit as usize)
        .count();
    Ok(n as u32)
}

#[screen(route = "/plain", permission = "admin.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/history", permission = "admin.read")]
pub fn history() -> Screen {
    Screen::new("History").body(DiffView::of(
        &account("A", Stage::Draft),
        &account("A", Stage::Approved),
    ))
}

#[screen(route = "/labels", permission = "admin.read")]
pub fn labels() -> Screen {
    Screen::new("Labels")
        .body(Barcode::code128("SO-1042"))
        .body(CodeView::new(CodeLang::Json, "{}"))
}

#[screen(route = "/inbox", permission = "admin.read")]
pub fn inbox() -> Screen {
    Screen::new("Inbox").body(
        Card::new().add(
            Inbox::of(search_notes)
                .title("Inbox")
                .when(Note::created_at)
                .read(Note::read)
                .item(|n: &Note| ListItem::new(n.title.clone()))
                .mark_read(mark_notes_read)
                .archive(archive_notes),
        ),
    )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app(notes: Notes) -> axum::Router {
    App::new("Admin")
        .i18n(I18n::new("en"))
        .service(notes)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
        .feature(
            Feature::new("admin")
                .screen(plain)
                .screen(history)
                .screen(labels)
                .screen(inbox),
        )
        .router()
        .unwrap()
}

const ALL: &str = "admin.read,notes.read,notes.write";

async fn body(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_as(router: &axum::Router, path: &str, perms: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-perms", perms)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

async fn get(path: &str) -> (StatusCode, String) {
    get_as(&app(Notes::seeded()), path, ALL).await
}

/// Calls `POST /_zeyn/data`, with the CSRF header unless `csrf` is `Csrf::Missing`.
async fn data(
    router: &axum::Router,
    perms: &str,
    csrf: Csrf,
    call: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf == Csrf::Sent {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = router
        .clone()
        .oneshot(
            req.body(Body::from(call.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

#[derive(PartialEq, Clone, Copy)]
enum Csrf {
    Sent,
    Missing,
}

#[tokio::test]
async fn admin_stylesheets_and_scripts_load_only_on_pages_that_use_them() {
    let router = app(Notes::seeded());
    let (_, text) = get_as(&router, "/plain", "admin.read").await;
    for name in ["diff.css", "code.css", "barcode.css", "topics.js"] {
        assert!(!text.contains(&format!("/_zeyn/{name}")), "{name}");
    }
    assert!(
        text.contains("/_zeyn/inbox.js?v=") && text.contains("/_zeyn/inbox.css?v="),
        "the bell of the top bar is on every page and brings its module"
    );
    let bare = App::new("Bare")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["admin.read"])))
        .feature(Feature::new("admin").screen(plain))
        .router()
        .unwrap();
    let (_, text) = get_as(&bare, "/plain", "admin.read").await;
    assert!(!text.contains("/_zeyn/inbox."), "no bell, no inbox module");
    let (status, page) = get("/history").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("/_zeyn/diff.css?v="));
    let (status, css) = get("/_zeyn/diff.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(css.contains(".z-diff-table"));
    let (_, shown) = get("/labels").await;
    assert!(shown.contains("/_zeyn/barcode.css?v=") && shown.contains("/_zeyn/code.css?v="));
    assert!(
        shown.contains("/_zeyn/share.js?v="),
        "the copy button brings share.js"
    );
    assert!(!shown.contains("/_zeyn/diff.css"));
}

#[tokio::test]
async fn the_inbox_groups_by_day_marks_unread_and_escapes() {
    let (status, page) = get("/inbox").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(">Today</h3>") && page.contains(">Yesterday</h3>"));
    assert_eq!(
        page.matches(r#"class="z-inbox-day""#)
            .count(),
        3
    );
    assert_eq!(
        page.matches("z-inbox-item z-inbox-unread")
            .count(),
        2
    );
    assert!(page.contains("2 unread"));
    assert!(page.contains("Invoice &lt;paid&gt;") && !page.contains("<paid>"));
    assert!(
        page.contains(r#"data-zeyn-inbox-run="read""#)
            && page.contains(r#"data-zeyn-inbox-run="archive""#)
    );
    assert!(page.contains(r#"data-zeyn-listens="notifications""#));
    assert!(page.contains("/_zeyn/inbox.js?v=") && page.contains("/_zeyn/inbox.css?v="));
    assert!(
        page.contains("/_zeyn/topics.js?v="),
        "the inbox refreshes through topics"
    );
    assert!(
        page.contains(r#"data-zeyn-notify="notifications""#),
        "the bell is in the top bar"
    );
    let (_, reader) = get_as(&app(Notes::seeded()), "/inbox", "admin.read,notes.read").await;
    assert!(
        !reader.contains("data-zeyn-inbox-run"),
        "no buttons without the write permission"
    );
    assert!(!reader.contains(r#"class="z-inbox-check""#), "{reader}");
}

#[tokio::test]
async fn bulk_actions_need_csrf_the_permission_and_a_bounded_selection() {
    let notes = Notes::seeded();
    let router = app(notes.clone());
    let call = |key: &str, ids: serde_json::Value| serde_json::json!({ "screen": "inbox", "params": null, "node": "0.0", "key": key, "input": { "ids": ids } });
    let (status, _) = data(
        &router,
        ALL,
        Csrf::Missing,
        call("read", serde_json::json!(["1"])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = data(
        &router,
        "admin.read,notes.read",
        Csrf::Sent,
        call("read", serde_json::json!(["1"])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let many: Vec<String> = (0..=zeyn::ui::MAX_INBOX)
        .map(|i| i.to_string())
        .collect();
    let (status, _) = data(
        &router,
        ALL,
        Csrf::Sent,
        call("read", serde_json::json!(many)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = data(
        &router,
        ALL,
        Csrf::Sent,
        call("read", serde_json::json!(["1", "3"])),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(r#""done":2"#));
    assert!(
        notes
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|n| n.read)
    );
    let (status, _) = data(
        &router,
        ALL,
        Csrf::Sent,
        call("archive", serde_json::json!(["2"])),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(notes.0.lock().unwrap().len(), 2);
    let (status, _) = data(
        &router,
        ALL,
        Csrf::Sent,
        call("delete", serde_json::json!(["1"])),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_bell_counts_unread_in_the_top_bar_with_the_actions_permission() {
    let router = app(Notes::seeded());
    let count = serde_json::json!({ "screen": "_app", "params": null, "node": "2.0", "key": "count", "input": null });
    let (status, body) = data(&router, ALL, Csrf::Sent, count.clone()).await;
    if status == StatusCode::NOT_FOUND {
        let (_, page) = get("/plain").await;
        panic!("the bell's node path is not 2: {page}");
    }
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains(r#""unread":2"#));
    let (status, _) = data(&router, "admin.read", Csrf::Sent, count).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[choice]
pub enum TaskState {
    Todo,
    Doing,
    Done,
}

#[entity(table = "tasks")]
pub struct Task {
    #[key]
    pub id: TaskId,
    pub title: String,
    pub state: TaskState,
    pub rank: Rank,
}

#[derive(Clone, Default)]
pub struct Tasks(Arc<Mutex<Vec<Task>>>);

impl Service for Tasks {}

impl Tasks {
    fn seeded() -> Tasks {
        let task = |id: i64, title: &str, state: TaskState| Task {
            id: TaskId(id),
            title: title.into(),
            state,
            rank: zeyn::rank::rebalance(4)[id as usize - 1].clone(),
        };
        Tasks(Arc::new(Mutex::new(vec![
            task(1, "Write <docs>", TaskState::Todo),
            task(2, "Review", TaskState::Doing),
            task(3, "Ship", TaskState::Doing),
            task(4, "Plan", TaskState::Done),
        ])))
    }
}

fn task_flow() -> Workflow<TaskState> {
    Workflow::new()
        .transition("start", TaskState::Todo, TaskState::Doing, "tasks.work")
        .transition("finish", TaskState::Doing, TaskState::Done, "tasks.work")
        .transition("reopen", TaskState::Done, TaskState::Todo, "tasks.admin")
}

#[action(permission = "tasks.read")]
pub async fn search_tasks(q: DataQuery, tasks: Tasks) -> Result<Page<Task>> {
    Ok(q.apply(tasks.0.lock().unwrap().clone()))
}

#[action(permission = "tasks.work")]
pub async fn move_task(r: TransitionRequest<TaskId>, tasks: Tasks, ctx: Ctx) -> Result<TaskId> {
    let mut all = tasks.0.lock().unwrap();
    let task = all
        .iter_mut()
        .find(|t| t.id == r.id)
        .ok_or(Error::NotFound)?;
    task.state = task_flow().apply(&ctx, task.state, &r.transition, r.comment.as_deref())?;
    Ok(task.id)
}

#[screen(route = "/board", permission = "admin.read")]
pub fn board() -> Screen {
    Screen::new("Board").body(
        Card::new().add(
            Kanban::of(task_flow(), search_tasks, move_task)
                .title("Tasks")
                .state(Task::state)
                .card(|t: &Task| Paragraph::new(t.title.clone()))
                .wip(TaskState::Doing, 1)
                .confirm("reopen", "Open it again?"),
        ),
    )
}

#[action(permission = "tasks.work")]
pub async fn reorder_task(r: ReorderRequest<TaskId>, tasks: Tasks) -> Result<()> {
    let mut all = tasks.0.lock().unwrap();
    let rank = r.rank(|k| {
        all.iter()
            .find(|t| t.id == *k)
            .map(|t| t.rank.clone())
    })?;
    let task = all
        .iter_mut()
        .find(|t| t.id == r.key)
        .ok_or(Error::NotFound)?;
    task.rank = rank;
    Ok(())
}

#[screen(route = "/ordered", permission = "admin.read")]
pub fn ordered_board() -> Screen {
    Screen::new("Ordered").body(
        Card::new().add(
            Kanban::of(task_flow(), search_tasks, move_task)
                .state(Task::state)
                .card(|t: &Task| Paragraph::new(t.title.clone()))
                .sortable(Task::rank, reorder_task),
        ),
    )
}

fn board_app(tasks: Tasks) -> axum::Router {
    App::new("Admin")
        .i18n(I18n::new("en"))
        .service(tasks)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(
            Feature::new("board")
                .screen(board)
                .screen(ordered_board),
        )
        .router()
        .unwrap()
}

const WORKER: &str = "admin.read,tasks.read,tasks.work";

#[tokio::test]
async fn a_kanban_has_a_column_per_state_with_counts_wip_and_the_users_moves() {
    let router = board_app(Tasks::seeded());
    let (status, page) = get_as(&router, "/board", WORKER).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page.matches(r#"<section class="z-kanban-col"#)
            .count(),
        3
    );
    assert!(page.contains(
        r#"class="z-kanban-col z-kanban-over" data-state="Doing" data-count="2" data-wip="1""#
    ));
    assert!(page.contains(">2 of 1</span>"));
    assert!(page.contains(r#"data-state="Todo" data-count="1""#));
    assert!(page.contains("Write &lt;docs&gt;") && !page.contains("<docs>"));
    assert_eq!(
        page.matches("data-zeyn-kanban-card")
            .count(),
        4
    );
    assert!(
        page.contains(r#"data-zeyn-node="0.0.1""#),
        "columns are nested nodes"
    );
    assert!(
        page.contains("&quot;Todo&quot;:[{&quot;confirm&quot;:null"),
        "{page}"
    );
    assert!(
        !page.contains("&quot;Done&quot;:["),
        "reopen needs tasks.admin"
    );
    assert!(
        page.contains("data-zeyn-confirm"),
        "the confirm dialog comes along"
    );
    for name in ["kanban.js", "kanban.css", "vlist.js", "components.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    let (_, admin) = get_as(
        &router,
        "/board",
        "admin.read,tasks.read,tasks.work,tasks.admin",
    )
    .await;
    assert!(admin.contains("&quot;Done&quot;:[{&quot;confirm&quot;:&quot;Open it again?&quot;"));
}

#[tokio::test]
async fn a_sortable_kanban_reorders_cards_inside_a_column_through_the_columns_node() {
    let tasks = Tasks::seeded();
    let router = board_app(tasks.clone());
    let (status, page) = get_as(&router, "/ordered", WORKER).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(
        page.matches("data-zeyn-sortable")
            .count(),
        3,
        "every column is a sortable list"
    );
    assert!(page.contains("z-sort-handle"));
    let call = |key: &str, after: Option<&str>, before: Option<&str>| serde_json::json!({ "screen": "ordered_board", "params": null, "node": "0.0.1", "key": "reorder", "input": { "key": key, "after": after, "before": before } });
    let (status, _) = data(&router, WORKER, Csrf::Missing, call("3", None, Some("2"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = data(
        &router,
        "admin.read,tasks.read",
        Csrf::Sent,
        call("3", None, Some("2")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the reorder action's permission"
    );
    let (status, _) = data(&router, WORKER, Csrf::Sent, call("3", None, Some("99"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an unknown neighbor");
    let (status, reply) = data(&router, WORKER, Csrf::Sent, call("3", None, Some("2"))).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let all = tasks.0.lock().unwrap();
    assert!(all[2].rank < all[1].rank, "Ship now comes before Review");
}

#[tokio::test]
async fn moving_a_card_runs_the_transition_with_its_permission_and_refuses_stale_moves() {
    let tasks = Tasks::seeded();
    let router = board_app(tasks.clone());
    let call = |key: &str, from: &str, to: &str| serde_json::json!({ "screen": "board", "params": null, "node": "0.0", "key": "move", "input": { "key": key, "from": from, "to": to } });
    let (status, _) = data(&router, WORKER, Csrf::Missing, call("1", "Todo", "Doing")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = data(
        &router,
        "admin.read,tasks.read",
        Csrf::Sent,
        call("1", "Todo", "Doing"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the move action's permission"
    );
    let (status, _) = data(&router, WORKER, Csrf::Sent, call("1", "Todo", "Done")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "no transition from Todo to Done"
    );
    let (status, _) = data(&router, WORKER, Csrf::Sent, call("4", "Done", "Todo")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reopen needs tasks.admin");
    let (status, reply) = data(&router, WORKER, Csrf::Sent, call("1", "Todo", "Doing")).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(tasks.0.lock().unwrap()[0].state, TaskState::Doing);
    let (status, _) = data(&router, WORKER, Csrf::Sent, call("1", "Todo", "Doing")).await;
    assert_eq!(status, StatusCode::CONFLICT, "the card moved already");
    let (status, _) = data(&router, WORKER, Csrf::Sent, call("99", "Todo", "Doing")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let res = router
        .clone()
        .oneshot(
            Request::post("/_zeyn/nodes")
                .header("x-perms", WORKER)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
                .header("x-zeyn-csrf", TOKEN)
                .body(Body::from(
                    serde_json::json!({ "screen": "board", "params": null, "nodes": ["0.0.1"] })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, html) = body(res).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(r#"data-zeyn-key=\"1\""#) && html.contains("Write &lt;docs&gt;"),
        "{html}"
    );
}

#[entity(table = "events")]
pub struct Meeting {
    #[key]
    pub id: MeetingId,
    pub title: String,
    pub starts: DateTime,
    pub ends: DateTime,
}

#[params]
pub struct MeetingParams {
    pub id: i64,
}

#[derive(Clone, Default)]
pub struct Meetings {
    rows: Arc<Mutex<Vec<Meeting>>>,
    queries: Arc<Mutex<Vec<DataQuery>>>,
    slots: Arc<Mutex<Vec<CalendarSlot>>>,
}

impl Service for Meetings {}

fn at(s: &str) -> DateTime {
    DateTime::parse(s).unwrap()
}

impl Meetings {
    fn seeded() -> Meetings {
        let m = |id: i64, title: &str, a: &str, b: &str| Meeting {
            id: MeetingId(id),
            title: title.into(),
            starts: at(a),
            ends: at(b),
        };
        let store = Meetings::default();
        *store.rows.lock().unwrap() = vec![
            m(
                1,
                "Stand up",
                "2026-11-06T08:00:00Z",
                "2026-11-06T08:15:00Z",
            ),
            m(
                2,
                "Late <call>",
                "2026-11-06T23:30:00Z",
                "2026-11-07T00:30:00Z",
            ),
            m(3, "Trip", "2026-11-09T00:00:00Z", "2026-11-11T12:00:00Z"),
            m(
                4,
                "Next year",
                "2027-01-20T09:00:00Z",
                "2027-01-20T10:00:00Z",
            ),
        ];
        store
    }
}

#[action(permission = "events.read")]
pub async fn search_meetings(q: DataQuery, store: Meetings) -> Result<Page<Meeting>> {
    store
        .queries
        .lock()
        .unwrap()
        .push(q.clone());
    Ok(q.apply(store.rows.lock().unwrap().clone()))
}

#[action(permission = "events.write")]
pub async fn new_meeting(slot: CalendarSlot, store: Meetings) -> Result<Option<Effect>> {
    store.slots.lock().unwrap().push(slot);
    Ok(Some(Window::open(meeting, MeetingParams { id: 9 })))
}

#[screen(permission = "events.read")]
pub fn meeting(p: MeetingParams) -> Screen {
    Screen::window(format!("Meeting {}", p.id))
}

#[screen(route = "/planner", permission = "admin.read")]
pub fn planner(p: CalendarParams) -> Screen {
    Screen::new("Planner").body(
        Card::new().add(
            Calendar::of(search_meetings)
                .start(Meeting::starts)
                .end(Meeting::ends)
                .label(|m: &Meeting| m.title.clone())
                .open(|m: &Meeting| Window::open(meeting, MeetingParams { id: m.id.0 }))
                .create(new_meeting)
                .at(p),
        ),
    )
}

fn planner_app(store: Meetings) -> axum::Router {
    App::new("Admin")
        .i18n(I18n::new("en"))
        .service(store)
        .time_policy(TimePolicy::Organization(
            TimeZone::named("Europe/Berlin").unwrap(),
        ))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(
            Feature::new("planner")
                .screen(planner)
                .screen(meeting),
        )
        .router()
        .unwrap()
}

const PLANNER: &str = "admin.read,events.read,events.write";

#[tokio::test]
async fn the_month_view_places_events_by_day_in_the_users_zone_and_asks_only_for_the_range() {
    let store = Meetings::seeded();
    let router = planner_app(store.clone());
    let (status, page) = get_as(
        &router,
        "/planner?params=%7B%22view%22%3A%22month%22%2C%22date%22%3A%222026-11-06%22%7D",
        PLANNER,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(r#"role="grid""#) && page.contains(">November 2026</h2>"));
    assert_eq!(
        page.matches(r#"role="gridcell""#)
            .count(),
        42
    );
    let day7 = page
        .split(r#"data-day="2026-11-07""#)
        .nth(1)
        .unwrap()
        .split("</td>")
        .next()
        .unwrap();
    assert!(
        day7.contains("Late &lt;call&gt;"),
        "23:30 UTC is the 7th in Berlin"
    );
    assert!(day7.contains(">12:30 AM</time>"), "{day7}");
    let trip_days = page.matches(">Trip</span>").count();
    assert_eq!(trip_days, 3, "a multi day event shows on each of its days");
    assert!(!page.contains("Next year"));
    assert!(page.contains(r#"data-day="2026-11-06" tabindex="0""#));
    assert!(page.contains(r#"data-zeyn-effect=""#) && page.contains("open_window"));
    assert!(
        page.contains(r#"data-zeyn-drives="calendar""#)
            && page.contains(r#"data-zeyn-listens="calendar""#)
    );
    assert!(page.contains(" data-create"));
    for name in ["calendar.js", "calendar.css", "topics.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    let q = store
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert_eq!(q.filters.len(), 2);
    assert_eq!(q.filters[0].field, "starts");
    assert_eq!(q.filters[0].to.as_deref(), Some("2026-12-12"));
    assert_eq!(q.filters[1].field, "ends");
    assert_eq!(q.filters[1].from.as_deref(), Some("2026-11-01"));
    assert_eq!(q.time_zone.name(), "Europe/Berlin");
    let (_, reader) = get_as(&router, "/planner", "admin.read,events.read").await;
    assert!(
        !reader.contains(" data-create"),
        "no slots without the create permission"
    );
}

#[tokio::test]
async fn the_week_view_has_hour_rows_and_comes_back_through_the_nodes_endpoint() {
    let router = planner_app(Meetings::seeded());
    let res = router
        .clone()
        .oneshot(
            Request::post("/_zeyn/nodes")
                .header("x-perms", PLANNER)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
                .header("x-zeyn-csrf", TOKEN)
                .body(Body::from(
                    serde_json::json!({ "screen": "planner", "params": { "view": "week", "date": "2026-11-06" }, "nodes": ["0.0"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, json) = body(res).await;
    assert_eq!(status, StatusCode::OK);
    let html: serde_json::Value = serde_json::from_str(&json).unwrap();
    let html = html["nodes"]["0.0"].as_str().unwrap();
    assert!(html.contains("z-cal-week"));
    assert_eq!(
        html.matches(r#"data-slot="2026-11-06T"#)
            .count(),
        14
    );
    assert!(
        html.contains(r#"data-slot="2026-11-06T09:00""#),
        "08:00 UTC is 09:00 in Berlin"
    );
    assert!(
        html.contains(r#"data-slot="2026-11-01T07:00""#)
            && html.contains(r#"data-slot="2026-11-07T20:00""#),
        "en weeks start on Sunday"
    );
    assert!(html.contains(r#"aria-current="page">Week</a>"#));
}

#[tokio::test]
async fn creating_by_a_slot_reads_local_times_on_the_server_with_csrf_and_the_permission() {
    let store = Meetings::seeded();
    let router = planner_app(store.clone());
    let call = |start: &str, end: &str, all_day: bool| serde_json::json!({ "screen": "planner", "params": null, "node": "0.0", "key": "create", "input": { "start": start, "end": end, "all_day": all_day } });
    let (status, _) = data(
        &router,
        PLANNER,
        Csrf::Missing,
        call("2026-11-06T10:00", "2026-11-06T11:00", false),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = data(
        &router,
        "admin.read,events.read",
        Csrf::Sent,
        call("2026-11-06T10:00", "2026-11-06T11:00", false),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, reply) = data(
        &router,
        PLANNER,
        Csrf::Sent,
        call("2026-11-06T10:00", "2026-11-06T11:00", false),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert!(reply.contains("open_window") && reply.contains(r#""id":9"#));
    assert_eq!(
        store.slots.lock().unwrap()[0],
        CalendarSlot {
            start: at("2026-11-06T09:00:00Z"),
            end: at("2026-11-06T10:00:00Z"),
            all_day: false
        }
    );
    let (status, _) = data(
        &router,
        PLANNER,
        Csrf::Sent,
        call("2026-11-06", "2026-11-07", true),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let day = store.slots.lock().unwrap()[1];
    assert_eq!(
        (day.start, day.end),
        (at("2026-11-05T23:00:00Z"), at("2026-11-07T23:00:00Z"))
    );
    let (status, _) = data(
        &router,
        PLANNER,
        Csrf::Sent,
        call("2026-03-29T02:00", "2026-03-29T03:00", false),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "a skipped hour");
    let (status, _) = data(
        &router,
        PLANNER,
        Csrf::Sent,
        call("2026-11-06T11:00", "2026-11-06T10:00", false),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = data(
        &router,
        PLANNER,
        Csrf::Sent,
        call("2026-01-01", "2026-03-01", true),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "more than 31 days"
    );
}
