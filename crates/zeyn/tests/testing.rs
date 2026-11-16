//! `zeyn::testing::TestApp`: screens, forms, grids and API actions driven in memory with the
//! app's own access rules.

use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
use zeyn::testing::TestApp;
use zeyn::testing::json;

#[entity(table = "tasks")]
pub struct Task {
    #[key]
    pub id: TaskId,
    #[rule(required, max_len = 40)]
    pub title: String,
}

#[derive(Clone, Default)]
pub struct Tasks(Arc<RwLock<Vec<Task>>>);

impl Service for Tasks {}

#[action(permission = "tasks.read")]
pub async fn search_tasks(q: DataQuery, tasks: Tasks) -> Result<Page<Task>> {
    Ok(q.apply(tasks.0.read().unwrap().clone()))
}

#[action(permission = "tasks.write")]
pub async fn save_task(t: Valid<Task>, tasks: Tasks) -> Result<Task> {
    let task = t.into_inner();
    tasks
        .0
        .write()
        .unwrap()
        .push(task.clone());
    Ok(task)
}

#[screen(route = "/tasks", permission = "tasks.read")]
pub fn task_list() -> Screen {
    Screen::new("Tasks").body(
        DataGrid::of(search_tasks)
            .column(Task::title)
            .filter(),
    )
}

#[screen(route = "/tasks/new", permission = "tasks.write")]
pub fn task_new() -> Screen {
    Screen::new("New task").body(
        Form::of(Task {
            id: TaskId(1),
            title: String::new(),
        })
        .field(Task::title)
        .submit("Save", save_task)
        .on_success(|_, ui| {
            ui.toast("Saved");
            ui.refresh("tasks");
        }),
    )
}

fn app() -> App {
    App::new("Tasks")
        .service(Tasks::default())
        .feature(
            Feature::new("tasks")
                .screen(task_list)
                .screen(task_new)
                .api(save_task),
        )
}

fn writer() -> Identity {
    Identity::user("ayse").with_permissions(["tasks.read", "tasks.write"])
}

#[tokio::test]
async fn a_signed_in_user_opens_a_screen_saves_a_form_and_pages_the_grid() {
    let app = TestApp::signed_in(app(), writer());
    let page = app.get("/tasks/new").await;
    assert_eq!(page.status, 200);
    assert!(page.body.contains("New task"));

    let saved = app
        .submit(
            "task_new",
            json!(null),
            "0",
            json!({ "title": "Write tests" }),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.effects(), ["toast", "refresh"]);

    let rows = app
        .grid("task_list", json!(null), "0", json!({}))
        .await;
    assert_eq!(rows.status, 200, "{}", rows.body);
    assert!(
        rows.json()["rows"]
            .as_str()
            .unwrap()
            .contains("Write tests")
    );
}

#[tokio::test]
async fn rule_errors_come_back_by_field() {
    let app = TestApp::signed_in(app(), writer());
    let r = app
        .submit("task_new", json!(null), "0", json!({ "title": "" }))
        .await;
    assert_eq!(r.status, 422);
    assert_eq!(r.json()["errors"][0]["field"], "title");
}

#[tokio::test]
async fn access_rules_apply_to_pages_forms_and_the_api() {
    let reader = TestApp::signed_in(
        app(),
        Identity::user("ali").with_permissions(["tasks.read"]),
    );
    assert_eq!(reader.get("/tasks").await.status, 200);
    assert_eq!(reader.get("/tasks/new").await.status, 403);
    let denied = reader
        .submit("task_new", json!(null), "0", json!({ "title": "No" }))
        .await;
    assert_eq!(denied.status, 403);
    let api = reader
        .api("tasks", "save_task", json!({ "id": 2, "title": "No" }))
        .await;
    assert_eq!(api.status, 403);

    let writer = TestApp::signed_in(app(), writer());
    let api = writer
        .api("tasks", "save_task", json!({ "id": 2, "title": "Yes" }))
        .await;
    assert_eq!(api.status, 200, "{}", api.body);
    assert_eq!(api.json()["title"], "Yes");
}

#[test]
#[should_panic(expected = "the app has problems")]
fn an_app_with_problems_does_not_start() {
    TestApp::new(
        App::new("Broken")
            .service(Tasks::default())
            .feature(
                Feature::new("tasks")
                    .screen(task_list)
                    .screen(task_list),
            ),
    );
}

#[tokio::test]
async fn headers_reach_the_apps_own_sign_in() {
    let app = || app().auth(|h: &axum::http::HeaderMap| h.contains_key("x-user").then(writer));
    assert_eq!(
        TestApp::new(app())
            .get("/tasks")
            .await
            .status,
        403
    );
    let signed = TestApp::new(app()).header("x-user", "ayse");
    assert_eq!(signed.get("/tasks").await.status, 200);
}
