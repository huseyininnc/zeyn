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
pub struct Tree {
    label: Text,
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
            source: Box::new(ActionSource::<A>::new()),
            action: A::META,
        }
    }

    /// The tree's accessible name.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }
}

impl Component for Tree {
    fn render(&self, out: &mut String, r: &Render) {
        let texts = json!({
            "loading": r.i18n.text(r.locale, &Text::key("zeyn.ui.loading")),
            "empty": r.i18n.text(r.locale, &Text::key("zeyn.ui.tree.empty")),
            "failed": r.i18n.text(r.locale, &Text::key("zeyn.ui.load_failed")),
        });
        out.push_str(&format!(
            r#"<div class="z-tree" data-zeyn-tree data-texts="{}" data-max="{MAX_TREE_CHILDREN}"><ul role="tree" aria-label="{}" data-testid="{}.tree" aria-busy="true"><li class="z-tree-note">{}</li></ul></div>"#,
            attr(&texts),
            r.text(&self.label),
            r.scope,
            r.text(&Text::key("zeyn.ui.loading"))
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "tree", "label": self.label, "action": self.action })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        key.is_empty()
            .then_some(self.source.as_ref())
    }
}
