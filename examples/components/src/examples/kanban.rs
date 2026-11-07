//! Kanban examples: tasks in columns of a workflow's states, moved by drag, menu or keys. The
//! sample tasks are shared by every visitor; the catalog lets every visitor make the moves
//! (`catalog.demo`).

use crate::{Entry, ex};
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;

/// The states of a sample task.
#[choice]
pub enum TaskState {
    Backlog,
    Doing,
    Review,
    Done,
}

/// A task of the sample board.
#[entity(table = "tasks")]
pub struct Task {
    #[key]
    pub id: TaskId,
    pub title: String,
    pub owner: String,
    pub state: TaskState,
}

/// The moves of a sample task and who may make them.
pub fn task_flow() -> Workflow<TaskState> {
    Workflow::new()
        .transition(
            "start",
            TaskState::Backlog,
            TaskState::Doing,
            "catalog.demo",
        )
        .transition(
            "submit",
            TaskState::Doing,
            TaskState::Review,
            "catalog.demo",
        )
        .transition(
            "return",
            TaskState::Review,
            TaskState::Doing,
            "catalog.demo",
        )
        .transition(
            "approve",
            TaskState::Review,
            TaskState::Done,
            "catalog.demo",
        )
        .transition(
            "reopen",
            TaskState::Done,
            TaskState::Backlog,
            "catalog.demo",
        )
}

fn sample() -> Vec<Task> {
    let titles = [
        "Price list for spring",
        "Fix the invoice footer",
        "Import the new suppliers",
        "Stock count in Warehouse B",
        "Translate the help pages",
        "Review the refund policy",
        "Photos for the tea range",
        "Close the October books",
    ];
    let owners = ["Ada", "Alan", "Grace", "Linus"];
    let states = [
        TaskState::Backlog,
        TaskState::Backlog,
        TaskState::Backlog,
        TaskState::Doing,
        TaskState::Doing,
        TaskState::Review,
        TaskState::Done,
        TaskState::Done,
    ];
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| Task {
            id: TaskId(i as i64 + 1),
            title: title.to_string(),
            owner: owners[i % owners.len()].to_string(),
            state: states[i],
        })
        .collect()
}

/// Most sample tasks the shared store holds. Visitors only move them, so it never grows.
pub const MAX_SAMPLE_TASKS: usize = 8;

static TASKS: LazyLock<Mutex<Vec<Task>>> = LazyLock::new(|| Mutex::new(sample()));

/// How many sample tasks the shared store holds now.
pub fn stored() -> usize {
    TASKS
        .lock()
        .map(|t| t.len())
        .unwrap_or(0)
}

/// The sample tasks, in the scope of a column.
#[action(public)]
pub async fn sample_tasks(q: DataQuery) -> Result<Page<Task>> {
    let all = TASKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    Ok(q.apply(all))
}

/// Moves a sample task with the workflow, which checks its state and the permission.
#[action(public)]
pub async fn move_sample_task(r: TransitionRequest<TaskId>, ctx: Ctx) -> Result<TaskId> {
    let mut all = TASKS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let task = all
        .iter_mut()
        .find(|t| t.id == r.id)
        .ok_or(Error::NotFound)?;
    task.state = task_flow().apply(&ctx, task.state, &r.transition, None)?;
    Ok(task.id)
}

pub fn overview(_ctx: &Ctx) -> Node {
    Kanban::of(task_flow(), sample_tasks, move_sample_task)
        .title("Sprint 14")
        .state(Task::state)
        .card(|t: &Task| {
            Stack::new()
                .add(Paragraph::new(t.title.clone()))
                .add(Badge::new(t.owner.clone()))
        })
        .wip(TaskState::Doing, 2)
        .confirm("reopen", "Open this task again?")
        .into()
}

/// What the catalog shows of `Kanban`.
pub const ENTRY: Entry = Entry {
    name: "Kanban",
    docs: "components/admin#kanban",
    api: &["Kanban"],
    overview: &[ex!(overview)],
    examples: &[],
};
