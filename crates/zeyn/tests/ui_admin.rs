//! Admin components: DiffView, CodeView and JsonView, barcodes, the inbox and the notification
//! center, Kanban boards and calendars.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    Barcode, CodeLang, CodeView, DiffLayout, DiffView, Inbox, InboxSelection, JsonView, ListItem,
    MAX_CODE_LINES, NotificationCenter, QrCode, QrLevel, QrMatrix, UnreadCount, code128_symbols,
    highlight, qr_error_correction, qr_format_bits, qr_version_bits,
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
        .feature(Feature::new("board").screen(board))
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
