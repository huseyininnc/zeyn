//! Tree view (ARIA tree) whose children load from an action when a branch first opens.

use super::{ActionSource, DataSource, layout::into_node};
use crate::action::{Action, Result};
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

impl Component for Tree {
    fn render(&self, out: &mut String, r: &Render) {
        let text = |key: &'static str| r.i18n.text(r.locale, &Text::key(key));
        let mut texts = json!({
            "loading": text("zeyn.ui.loading"),
            "empty": text("zeyn.ui.tree.empty"),
            "failed": text("zeyn.ui.load_failed"),
        });
        if self.draggable {
            texts["insert"] = text("zeyn.ui.tree.insert").into();
            texts["insert_label"] = text("zeyn.ui.tree.insert_label").into();
            texts["inserted"] = text("zeyn.ui.tree.inserted").into();
        }
        out.push_str(&format!(
            r#"<div class="z-tree" data-zeyn-tree{} data-texts="{}" data-max="{MAX_TREE_CHILDREN}"><ul role="tree" aria-label="{}" data-testid="{}.tree" aria-busy="true"><li class="z-tree-note">{}</li></ul>"#,
            if self.draggable {
                " data-zeyn-tree-drag"
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
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "tree",
            "label": self.label,
            "action": self.action,
            "draggable": self.draggable,
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        key.is_empty()
            .then_some(self.source.as_ref())
    }
}
