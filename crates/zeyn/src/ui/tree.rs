//! Tree view (ARIA tree) whose children load from an action when a branch first opens.

use super::{ActionSource, DataSource, layout::into_node};
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::render::{Render, attr};
use crate::screen::{Component, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

into_node!(Tree);

/// Most children one branch may show.
pub const MAX_TREE_CHILDREN: usize = 1000;

/// What a tree action receives: the branch to list, or `None` for the top level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeQuery {
    #[serde(default)]
    pub parent: Option<String>,
}

/// Deepest a tree may be for [`TreeMove::parent`]: a walk up from the target that is longer
/// than this is refused, so a broken parent chain cannot loop.
pub const MAX_TREE_DEPTH: usize = 256;

/// Where a moved item lands next to its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TreePlace {
    /// Before the target, under the target's parent.
    Before,
    /// Inside the target, as its last child.
    Inside,
    /// After the target, under the target's parent.
    After,
}

/// What the move action of a [`Tree::movable`] tree receives: item `id` goes `place` of
/// `target`. Both ids come from the browser, so the action looks them up, checks them and
/// calls [`TreeMove::parent`], which refuses a move into the item itself or one of its
/// descendants.
///
/// ```
/// use zeyn::ui::{TreeMove, TreePlace};
/// let parents = [("b", "a"), ("c", "b")];
/// let parent_of = |id: &str| {
///     parents
///         .iter()
///         .find(|p| p.0 == id)
///         .map(|p| p.1.to_string())
/// };
/// let up = TreeMove { id: "c".into(), target: "a".into(), place: TreePlace::After };
/// assert_eq!(up.parent(parent_of).unwrap(), None);
/// let cycle = TreeMove { id: "a".into(), target: "c".into(), place: TreePlace::Inside };
/// assert!(cycle.parent(parent_of).is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeMove {
    /// The item that moves.
    pub id: String,
    /// The item it lands next to or inside.
    pub target: String,
    /// Where it lands.
    pub place: TreePlace,
}

impl TreeMove {
    /// The new parent of the moved item (`None` for the top level), refused with
    /// `Error::Conflict` when it would be the item itself or one of its descendants, or when
    /// the chain of parents is longer than [`MAX_TREE_DEPTH`]. `parent_of` gives the stored
    /// parent of an item, `None` at the top level.
    pub fn parent(
        &self,
        mut parent_of: impl FnMut(&str) -> Option<String>,
    ) -> Result<Option<String>> {
        let parent = match self.place {
            TreePlace::Inside => Some(self.target.clone()),
            TreePlace::Before | TreePlace::After => parent_of(&self.target),
        };
        let mut at = parent.clone();
        for _ in 0..MAX_TREE_DEPTH {
            match at {
                None => return Ok(parent),
                Some(ref id) if *id == self.id => return Err(Error::Conflict),
                Some(id) => at = parent_of(&id),
            }
        }
        Err(Error::Conflict)
    }
}

/// Longest item id a move accepts; longer ids are refused before the action runs.
const MAX_TREE_ID: usize = 200;

/// Runs the move action of a movable tree after checking the shape of the call.
struct MoveSource<A> {
    _action: std::marker::PhantomData<fn() -> A>,
}

impl<A> DataSource for MoveSource<A>
where
    A: Action<Input = TreeMove>,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let m: TreeMove = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            let bad = |id: &str| id.is_empty() || id.len() > MAX_TREE_ID;
            if bad(&m.id) || bad(&m.target) || m.id == m.target {
                return Err(Error::NotFound);
            }
            let output = A::call(ctx, m).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            Ok(json!({ "moved": true }))
        })
    }
}

/// One branch or leaf of a tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    /// Whether the item can open. Children load the first time it opens.
    pub has_children: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl TreeItem {
    pub fn branch(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            has_children: true,
            hint: None,
        }
    }

    pub fn leaf(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            has_children: false,
            hint: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// A tree. Keyboard (ARIA tree pattern): Up and Down move, Right opens or goes to the first
/// child, Left closes or goes to the parent, Home and End, Enter or Space selects, letters jump.
///
/// ```ignore
/// Tree::of(category_children).label("Categories")
/// ```
///
/// With [`Tree::draggable`] leaves can be dragged onto a drop target, such as an island.
pub struct Tree {
    label: Text,
    draggable: bool,
    mover: Option<Box<dyn DataSource>>,
    source: Box<dyn DataSource>,
    action: crate::action::ActionMeta,
}

impl Tree {
    pub fn of<A>(_action: A) -> Self
    where
        A: Action<Input = TreeQuery, Output = Result<Vec<TreeItem>>>,
    {
        Self {
            label: Text::from(""),
            draggable: false,
            mover: None,
            source: Box::new(ActionSource::<A>::new()),
            action: A::META,
        }
    }

    /// The tree's accessible name.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }

    /// Leaves can be dragged with HTML drag and drop. The drag carries the MIME type
    /// `application/x-zeyn-tree` with the JSON `{ "id", "path", "label" }` (`path` lists the ids
    /// from the top level down to the leaf) and the label as `text/plain`; a drop target sets
    /// `dropEffect = "copy"` in `dragover`. Without dragging, the Insert button on the focused
    /// leaf (a single pointer alternative) and Control+Enter or Command+Enter (keyboard) dispatch
    /// the cancelable DOM event `zeyn:tree-insert` with the same object as `detail`. A target
    /// that inserts at its own position calls `preventDefault()`, and only then does the tree
    /// announce the insert in its live region. Any page can forge the payload, so a target treats
    /// it as untrusted input and checks access on the server.
    ///
    /// ```ignore
    /// Tree::of(report_fields).label("Fields").draggable()
    /// ```
    pub fn draggable(mut self) -> Self {
        self.draggable = true;
        self
    }
}

impl Tree {
    /// People can move items of the tree: drag one with a mouse or pen onto the top, the middle
    /// (a branch only) or the bottom of another item's row, or use the Move button on the
    /// focused item (a menu: up, down, out of its branch, into the branch before it), or Alt
    /// with an arrow key. `move_item` gets a [`TreeMove`] through `POST /_zeyn/data` (CSRF, the
    /// write limit and its permission); it looks both ids up and stores the move. Call
    /// [`TreeMove::parent`] for the new parent: it refuses a move into the item itself or one of
    /// its descendants, which the browser cannot be trusted to check. After a move the tree
    /// loads the changed levels again and keeps the moved item focused.
    ///
    /// ```ignore
    /// #[action(permission = "folders.write")]
    /// pub async fn move_folder(m: TreeMove, db: Db) -> Result<()> {
    ///     let parent = m.parent(|id| db.parent_of(id))?;
    ///     db.move_folder(&m.id, parent, &m.target, m.place).await
    /// }
    ///
    /// Tree::of(folders).label("Folders").movable(move_folder)
    /// ```
    ///
    /// The move action takes a [`TreeMove`]:
    ///
    /// ```compile_fail
    /// use zeyn::prelude::*;
    ///
    /// #[action(public)]
    /// pub async fn folders(q: TreeQuery) -> Result<Vec<TreeItem>> { Ok(Vec::new()) }
    ///
    /// #[action(public)]
    /// pub async fn rename(name: String) -> Result<()> { Ok(()) }
    ///
    /// let tree = Tree::of(folders).movable(rename);
    /// ```
    pub fn movable<A>(mut self, _move_item: A) -> Self
    where
        A: Action<Input = TreeMove>,
        A::Output: Outcome,
    {
        self.mover = Some(Box::new(MoveSource::<A> {
            _action: std::marker::PhantomData,
        }));
        self
    }
}

impl Component for Tree {
    fn render(&self, out: &mut String, r: &Render) {
        let text = |key: &'static str| r.i18n.text(r.locale, &Text::key(key));
        let mut texts = json!({
            "loading": text("zeyn.ui.loading"),
            "empty": text("zeyn.ui.tree.empty"),
            "failed": text("zeyn.ui.load_failed"),
        });
        if self.mover.is_some() {
            for (name, key) in [
                ("move", "zeyn.ui.tree.move"),
                ("move_label", "zeyn.ui.tree.move_label"),
                ("move_up", "zeyn.ui.tree.move_up"),
                ("move_down", "zeyn.ui.tree.move_down"),
                ("move_out", "zeyn.ui.tree.move_out"),
                ("move_into", "zeyn.ui.tree.move_into"),
                ("move_none", "zeyn.ui.tree.move_none"),
                ("moved", "zeyn.ui.tree.moved"),
                ("move_refused", "zeyn.ui.tree.move_refused"),
            ] {
                texts[name] = text(key).into();
            }
        }
        if self.draggable {
            texts["insert"] = text("zeyn.ui.tree.insert").into();
            texts["insert_label"] = text("zeyn.ui.tree.insert_label").into();
            texts["inserted"] = text("zeyn.ui.tree.inserted").into();
        }
        out.push_str(&format!(
            r#"<div class="z-tree{}" data-zeyn-tree{}{} data-texts="{}" data-max="{MAX_TREE_CHILDREN}"><ul role="tree" aria-label="{}" data-testid="{}.tree" aria-busy="true"><li class="z-tree-note">{}</li></ul>"#,
            if self.mover.is_some() {
                " z-tree-movable"
            } else {
                ""
            },
            if self.draggable {
                " data-zeyn-tree-drag"
            } else {
                ""
            },
            if self.mover.is_some() {
                " data-zeyn-tree-move"
            } else {
                ""
            },
            attr(&texts),
            r.text(&self.label),
            r.scope,
            r.text(&Text::key("zeyn.ui.loading"))
        ));
        if self.draggable {
            out.push_str(&format!(
                r#"<span class="z-sr-only z-tree-keys" aria-hidden="true">{}</span><span class="z-sr-only" aria-live="polite"></span>"#,
                r.text(&Text::key("zeyn.ui.tree.insert_hint"))
            ));
        }
        if self.mover.is_some() {
            out.push_str(&format!(
                r#"<span class="z-sr-only z-tree-move-keys" aria-hidden="true">{}</span>"#,
                r.text(&Text::key("zeyn.ui.tree.move_hint"))
            ));
            if !self.draggable {
                out.push_str(r#"<span class="z-sr-only" aria-live="polite"></span>"#);
            }
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "tree",
            "label": self.label,
            "action": self.action,
            "draggable": self.draggable,
            "movable": self.mover.is_some(),
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        match key {
            "" => Some(self.source.as_ref()),
            "move" => self.mover.as_deref(),
            _ => None,
        }
    }
}
