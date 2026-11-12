//! [`Kanban`]: the records of an action as cards in columns, one per state of a workflow.
//! Moving a card runs the workflow transition between the two states.

use super::catalog::CardGrid;
use super::layout::into_node;
use super::{DataSource, dom_id, render_child};
use crate::action::{Access, Action, ActionMeta, DataQuery, Error, Page, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::grid::{Paging, Tone};
use crate::i18n::Text;
use crate::model::{Field, Model};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node};
use crate::value::{FieldKind, Value};
use crate::workflow::{TransitionRequest, Workflow, WorkflowState};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value as Json, json};
use std::marker::PhantomData;
use std::sync::Arc;

/// Cards one column fetches at a time; the column is a virtual list, so only the cards in view
/// are in the page.
pub const KANBAN_BLOCK: u32 = 50;

/// One allowed move: the states as their choice values, the transition and its permission.
#[derive(Clone)]
struct Move {
    from: &'static str,
    to: &'static str,
    name: &'static str,
    permission: &'static str,
}

/// The choice value of a state.
fn value_of<S: WorkflowState>(state: S) -> &'static str {
    match state.to_value() {
        Value::Choice(v) => v,
        _ => "",
    }
}

/// Runs the transition between two columns for one card. The card key comes from the browser;
/// the move action loads the record and `Workflow::apply` checks its state, so a card someone
/// else moved first is refused with a conflict.
struct MoveSource<K, M> {
    moves: Arc<Vec<Move>>,
    _types: PhantomData<fn() -> (K, M)>,
}

/// What the browser sends to move a card.
#[derive(serde::Deserialize)]
struct MoveCall {
    key: String,
    from: String,
    to: String,
}

impl<K, M> DataSource for MoveSource<K, M>
where
    K: DeserializeOwned + Clone + Send + Sync + 'static,
    M: Action<Input = TransitionRequest<K>>,
    M::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        M::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let moves = self.moves.clone();
        Box::pin(async move {
            let call: MoveCall = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            if call.key.is_empty() || call.key.len() > 200 {
                return Err(Error::NotFound);
            }
            let found: Vec<&Move> = moves
                .iter()
                .filter(|m| m.from == call.from && m.to == call.to)
                .collect();
            let Some(m) = found.iter().find(|m| {
                ctx.identity()
                    .can(&Access::Permission(m.permission))
            }) else {
                return Err(match found.is_empty() {
                    true => Error::NotFound,
                    false => Error::Forbidden,
                });
            };
            let id: K = serde_json::from_value(Json::String(call.key.clone()))
                .or_else(|_| serde_json::from_str(&call.key))
                .map_err(|_| Error::NotFound)?;
            let request = TransitionRequest {
                id,
                transition: m.name.to_string(),
                comment: None,
            };
            let output = M::call(ctx, request).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            Ok(json!({ "moved": true }))
        })
    }
}

into_node!(KanbanCard);

/// A card as the board draws it: the app's node with a focus target and the move button.
struct KanbanCard {
    inner: Vec<Node>,
}

impl Component for KanbanCard {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(r#"<div class="z-kanban-card" tabindex="-1" data-zeyn-kanban-card>"#);
        for node in &self.inner {
            node.render(out, r);
        }
        out.push_str(&format!(
            r#"<button type="button" class="z-kanban-move" tabindex="-1" aria-haspopup="menu" aria-expanded="false" hidden>{}<span class="z-sr-only">{}</span></button></div>"#,
            crate::screen::Icon::MoreHorizontal.html(),
            r.text(&Text::key("zeyn.ui.kanban.move"))
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "kanban_card" })
    }
}

type CardFn<E> = Arc<dyn Fn(&E) -> Node + Send + Sync>;

/// Builds the columns of a board again after a builder method changed them.
type MakeFn<S, E> = Box<dyn Fn(&Kanban<S, E>) -> Vec<Node> + Send + Sync>;

/// The records of an action as cards in columns, one per state of a workflow's `#[choice]`
/// enum, in the enum's order. Each column is a virtual list over the cards in its state (the
/// action gets a `DataQuery` scoped to the state, so it filters in the database), fetched in
/// blocks of [`KANBAN_BLOCK`]; the column head shows the number of cards and the WIP limit.
///
/// Moving a card from one column to another runs the workflow transition between the two
/// states through the move action, with a [`TransitionRequest`] like [`crate::Transitions`]:
/// the server finds the transition, checks its permission, and the action applies it with
/// `Workflow::apply`, which refuses a card in another state (someone moved it first). The board
/// moves the card at once and puts it back with a message when the server refuses. Only moves
/// the user may make are offered; `.confirm(name, message)` asks first.
///
/// Pointer: drag a card with a mouse or pen. Keyboard and touch: the board is one tab stop;
/// arrow keys move between cards, Enter or Space opens the move menu (also the button on each
/// card), Shift with Left or Right moves the card to the column next to it.
///
/// ```ignore
/// Kanban::of(task_flow(), search_tasks, move_task)
///     .state(Task::state)
///     .card(|t: &Task| Stack::new().add(Heading::new(t.title.clone())).add(Badge::new(t.owner.clone())))
///     .wip(TaskState::Doing, 5)
///     .confirm("cancel", t!("tasks.cancel_confirm"))
/// ```
///
/// The state field must be the workflow's state type:
///
/// ```compile_fail
/// use zeyn::prelude::*;
/// use zeyn::ui::Kanban;
///
/// #[choice]
/// pub enum Stage { Todo, Done }
///
/// #[entity(table = "tasks")]
/// pub struct Task {
///     #[key]
///     pub id: TaskId,
///     pub title: String,
/// }
///
/// #[action(public)]
/// pub async fn tasks(q: DataQuery) -> Result<Page<Task>> { Ok(q.apply(Vec::new())) }
///
/// #[action(public)]
/// pub async fn move_task(r: TransitionRequest<TaskId>) -> Result<()> { Ok(()) }
///
/// let flow = Workflow::new().transition("finish", Stage::Todo, Stage::Done, "tasks.write");
/// let board = Kanban::of(flow, tasks, move_task).state(Task::title);
/// ```
pub struct Kanban<S, E: Model> {
    columns: Vec<Node>,
    states: Vec<(&'static str, Text)>,
    title: Option<Text>,
    wip: Vec<(&'static str, u32)>,
    asks: Vec<(&'static str, Text, bool)>,
    moves: Arc<Vec<Move>>,
    source: Box<dyn DataSource>,
    make: MakeFn<S, E>,
    card: Option<CardFn<E>>,
    state: Option<Field<E, S>>,
    row_height: u32,
    _state: PhantomData<fn() -> S>,
}

impl<S, E> Kanban<S, E>
where
    S: WorkflowState + DeserializeOwned,
    E: Model,
{
    /// A board over the cards of `cards`, moved with `move_card`, with a column for every state
    /// of `flow`'s state type.
    pub fn of<A, K, M>(flow: Workflow<S>, _cards: A, _move_card: M) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<Page<E>>> + Copy + Send + Sync + 'static,
        K: DeserializeOwned + Clone + Send + Sync + 'static,
        M: Action<Input = TransitionRequest<K>>,
        M::Output: Outcome,
    {
        let states = match S::KIND {
            FieldKind::Choice(options) => options
                .iter()
                .map(|o| (o.value, Text::key_or(o.key, o.label)))
                .collect(),
            _ => Vec::new(),
        };
        let moves: Vec<Move> = flow
            .transitions()
            .iter()
            .map(|t| Move {
                from: value_of(t.from),
                to: value_of(t.to),
                name: t.name,
                permission: t.permission,
            })
            .collect();
        let moves = Arc::new(moves);
        Kanban {
            columns: Vec::new(),
            states,
            title: None,
            wip: Vec::new(),
            asks: Vec::new(),
            moves: moves.clone(),
            source: Box::new(MoveSource::<K, M> {
                moves,
                _types: PhantomData,
            }),
            make: Box::new(move |board: &Kanban<S, E>| board.build(_cards)),
            card: None,
            state: None,
            row_height: 88,
            _state: PhantomData,
        }
    }

    /// The field that holds a record's state: each column shows the records in its state.
    pub fn state(mut self, field: Field<E, S>) -> Self {
        self.state = Some(field);
        self.columns = (self.make)(&self);
        self
    }

    /// How a record is drawn on its card. The card keeps the fixed height of
    /// [`Kanban::row_height`].
    pub fn card<N: Into<Node>>(mut self, build: impl Fn(&E) -> N + Send + Sync + 'static) -> Self {
        self.card = Some(Arc::new(move |row| build(row).into()));
        self.columns = (self.make)(&self);
        self
    }

    /// Height of every card in pixels, at least 48; 88 unless set.
    pub fn row_height(mut self, px: u32) -> Self {
        self.row_height = px.max(48);
        self.columns = (self.make)(&self);
        self
    }

    /// A heading over the board, which also names it for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The work in progress limit of a column, shown in its head; a column above it is marked.
    pub fn wip(mut self, state: S, limit: u32) -> Self {
        let v = value_of(state);
        self.wip.retain(|w| w.0 != v);
        self.wip.push((v, limit));
        self
    }

    /// Asks the user to confirm with this message before the transition `name` runs.
    pub fn confirm(mut self, name: &'static str, message: impl Into<Text>) -> Self {
        let danger = self
            .asks
            .iter()
            .any(|a| a.0 == name && a.2);
        self.asks.retain(|a| a.0 != name);
        self.asks
            .push((name, message.into(), danger));
        self
    }

    /// `Tone::Danger` draws the confirmation of transition `name` in red.
    pub fn tone(mut self, name: &'static str, tone: Tone) -> Self {
        let danger = tone == Tone::Danger;
        match self
            .asks
            .iter_mut()
            .find(|a| a.0 == name)
        {
            Some(a) => a.2 = danger,
            None => self
                .asks
                .push((name, Text::from(""), danger)),
        }
        self
    }

    /// One virtual list per column, scoped to its state.
    fn build<A>(&self, cards: A) -> Vec<Node>
    where
        A: Action<Input = DataQuery, Output = Result<Page<E>>> + Copy,
    {
        let (Some(field), Some(card)) = (self.state, self.card.clone()) else {
            return Vec::new();
        };
        self.states
            .iter()
            .filter_map(|(value, _)| {
                let state: S = serde_json::from_value(Json::String((*value).to_string())).ok()?;
                let card = card.clone();
                let grid = CardGrid::<E>::of(cards)
                    .into_list()
                    .scope(field, state)
                    .builder(move |row: &E| KanbanCard {
                        inner: vec![card(row)],
                    })
                    .row_height(self.row_height)
                    .paging(Paging::Virtual {
                        row_height: self.row_height,
                        block: KANBAN_BLOCK,
                    });
                Some(Node::from(grid))
            })
            .collect()
    }
}

impl<S, E> Component for Kanban<S, E>
where
    S: WorkflowState + DeserializeOwned,
    E: Model,
{
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.kanban.{key}")))
        };
        let id = dom_id(r, "kanban");
        let mut moves = Map::new();
        for m in self.moves.iter() {
            if !r.can(&Access::Permission(m.permission)) {
                continue;
            }
            let ask = self.asks.iter().find(|a| a.0 == m.name);
            let label = self
                .states
                .iter()
                .find(|s| s.0 == m.to)
                .map(|s| r.i18n.text(r.locale, &s.1))
                .unwrap_or_default();
            let confirm = ask
                .map(|a| r.i18n.text(r.locale, &a.1))
                .filter(|c| !c.is_empty());
            let entry = json!({
                "to": m.to,
                "label": label,
                "confirm": confirm,
                "danger": ask.is_some_and(|a| a.2),
            });
            if let Json::Array(list) = moves
                .entry(m.from)
                .or_insert_with(|| Json::Array(Vec::new()))
                && !list.iter().any(|e| e["to"] == m.to)
            {
                list.push(entry);
            }
        }
        let texts = json!({
            "move_to": t("move_to"),
            "moved": t("moved"),
            "refused": t("refused"),
            "none": t("no_moves"),
            "confirm_title": r.i18n.text(r.locale, &Text::key("zeyn.ui.confirm.title")),
            "cancel": r.i18n.text(r.locale, &Text::key("zeyn.ui.confirm.cancel")),
        });
        let confirms = self
            .asks
            .iter()
            .any(|a| !r.i18n.text(r.locale, &a.1).is_empty());
        let labelled = match &self.title {
            Some(_) => format!(r#" aria-labelledby="{id}-title""#),
            None => format!(r#" aria-label="{}""#, escape(&t("label"))),
        };
        out.push_str(&format!(
            r#"<section class="z-kanban" data-zeyn-kanban{}{labelled} aria-describedby="{id}-help" data-moves="{}" data-texts="{}" data-testid="{}.kanban" style="--z-kanban-h:{}px">"#,
            if confirms { " data-zeyn-confirm" } else { "" },
            attr(&Json::Object(moves)),
            attr(&texts),
            r.scope,
            self.row_height
        ));
        if let Some(title) = &self.title {
            out.push_str(&format!(
                r#"<h2 class="z-kanban-title" id="{id}-title">{}</h2>"#,
                r.text(title)
            ));
        }
        out.push_str(&format!(
            r#"<p class="z-sr-only" id="{id}-help">{}</p><div class="z-kanban-board">"#,
            escape(&t("help"))
        ));
        for (i, (value, label)) in self.states.iter().enumerate() {
            let Some(column) = self.columns.get(i) else {
                break;
            };
            let path = if r.path.is_empty() {
                i.to_string()
            } else {
                format!("{}.{i}", r.path)
            };
            let count = r
                .loaded
                .and_then(|l| l.get(&path))
                .and_then(|p| p.total)
                .unwrap_or(0);
            let wip = self
                .wip
                .iter()
                .find(|w| w.0 == *value)
                .map(|w| w.1);
            let over = wip.is_some_and(|w| count > u64::from(w));
            let shown = match wip {
                Some(w) => r.text(
                    &Text::key("zeyn.ui.kanban.count_wip")
                        .arg("n", count.to_string())
                        .arg("wip", w.to_string()),
                ),
                None => r.text(&Text::key("zeyn.ui.kanban.count").arg("n", count.to_string())),
            };
            out.push_str(&format!(
                r#"<section class="z-kanban-col{}" data-state="{}" data-count="{count}"{} aria-labelledby="{id}-c{i}"><header class="z-kanban-head"><h3 class="z-kanban-name" id="{id}-c{i}">{}</h3><span class="z-kanban-count" data-zeyn-kanban-count>{shown}</span></header>"#,
                if over { " z-kanban-over" } else { "" },
                escape(value),
                wip.map(|w| format!(r#" data-wip="{w}""#))
                    .unwrap_or_default(),
                r.text(label)
            ));
            render_child(out, r, i, column);
            out.push_str("</section>");
        }
        out.push_str(r#"</div><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-kanban-status></p></section>"#);
    }

    fn schema(&self) -> Json {
        json!({
            "type": "kanban",
            "model": E::NAME,
            "action": self.source.action(),
            "title": self.title,
            "states": self.states.iter().map(|s| s.0).collect::<Vec<_>>(),
            "moves": self.moves.iter().map(|m| json!({ "from": m.from, "to": m.to, "transition": m.name, "permission": m.permission })).collect::<Vec<_>>(),
            "wip": self.wip.iter().map(|w| json!({ "state": w.0, "limit": w.1 })).collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.columns
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "move").then_some(self.source.as_ref())
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        if self.state.is_none() {
            check.problem(
                crate::Problem::new(
                    "Z0617",
                    "a Kanban needs `.state(field)`, the field that holds a record's workflow state",
                )
                .why("the board puts each card in the column of its state, so without the field no card has a column")
                .fix("add `.state(field)` with the model's state field")
                .docs("components/admin#kanban"),
            );
        }
        if self.card.is_none() {
            check.problem(
                crate::Problem::new(
                    "Z0618",
                    "a Kanban needs `.card(|row| node)`, how a record is drawn on its card",
                )
                .why("the board does not know what to show on a card, so the cards would be empty")
                .fix("add `.card(|row| node)`")
                .docs("components/admin#kanban"),
            );
        }
    }
}

impl<S, E> From<Kanban<S, E>> for Node
where
    S: WorkflowState + DeserializeOwned,
    E: Model,
{
    fn from(k: Kanban<S, E>) -> Self {
        Node::new(k)
    }
}
