//! Component library: layout containers, field editors and display components.
//!
//! Containers hold other nodes. Every node has a path: the index of a body item (`"1"`), then the
//! index of each child below it (`"1.0.2"`). The renderer writes the path in `data-zeyn-node` and
//! the endpoints accept it, so grids and forms inside containers keep working.

mod chart;
mod controls;
mod date;
mod dock;
mod layout;
mod lines;
mod lookup;
mod shell;
mod tree;
mod widgets;

pub use layout::{Card, Panel, Row, Split, Stack, Tabs};
pub use shell::{MenuBar, MenuList, PropertyInspector, StatusBar, Toolbox};
pub use dock::{DOCK_SIZE, Dock, DockArea, DockLayout, DockPanelState, DockSizes};
pub(crate) use dock::panel_ids as dock_panel_ids;

pub use chart::{Chart, ChartKind};
pub use controls::Control;
pub use date::DateOptions;
pub use lines::{Lines, MAX_LINES};
pub use tree::{MAX_TREE_CHILDREN, Tree, TreeItem, TreeQuery};
pub use widgets::{ActionButton, Alert, Badge, Breadcrumb, DescriptionList, DescriptionOf, EmptyState, Progress, Skeleton, Timeline};
pub(crate) use lines::LinesEditor;
pub use lookup::{LookupItem, LookupQuery, LookupValue, MAX_LOOKUP_IDS, MAX_LOOKUP_PAGE};
pub(crate) use controls::{Slot, render_bool, render_choice, render_number, render_text};
pub(crate) use lookup::{parse_ids, render_lookup};
pub(crate) use date::{input_text, picker_config, range_reversed, render_date_input, render_range};

use crate::action::{Action, ActionMeta, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::render::Render;
use crate::screen::{Node, Screen};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as Json;
use std::fmt;
use std::marker::PhantomData;

/// Most levels a node path may have. Deeper screens are not useful and paths come from clients.
pub const MAX_NODE_DEPTH: usize = 16;

/// Address of a node in a screen: `"0"` for the first body item, `"1.0.2"` for a nested one.
///
/// Read from clients as a number (`0`, kept for older clients) or a dotted string.
///
/// ```
/// use zeyn::ui::NodePath;
/// let p: NodePath = serde_json::from_str("\"1.0.2\"").unwrap();
/// assert_eq!(p.as_str(), "1.0.2");
/// let old: NodePath = serde_json::from_str("3").unwrap();
/// assert_eq!(old.as_str(), "3");
/// assert!(serde_json::from_str::<NodePath>("\"1..2\"").is_err());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodePath(String);

impl NodePath {
    /// A checked path, or `None` when it is not dotted indexes.
    pub fn parse(s: &str) -> Option<NodePath> {
        let parts: Vec<&str> = s.split('.').collect();
        let valid = parts.len() <= MAX_NODE_DEPTH
            && parts.iter().all(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()));
        valid.then(|| NodePath(parts.iter().map(|p| p.parse::<usize>().unwrap_or(0).to_string()).collect::<Vec<_>>().join(".")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Indexes from the screen body down.
    pub fn indexes(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.split('.').filter_map(|p| p.parse().ok())
    }
}

impl From<usize> for NodePath {
    fn from(i: usize) -> Self {
        NodePath(i.to_string())
    }
}

impl fmt::Display for NodePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for NodePath {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NodePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Index(u64),
            Path(String),
        }
        let text = match Raw::deserialize(deserializer)? {
            Raw::Index(i) => i.to_string(),
            Raw::Path(s) => s,
        };
        NodePath::parse(&text).ok_or_else(|| serde::de::Error::custom("node must be an index or a dotted path such as 1.0.2"))
    }
}

impl Screen {
    /// The node at a path, following container children.
    pub fn node(&self, path: &NodePath) -> Option<&Node> {
        let mut indexes = path.indexes();
        let mut node = self.nodes().get(indexes.next()?)?;
        for i in indexes {
            node = node.children().get(i)?;
        }
        Some(node)
    }

    /// Every node rendered with the page, depth first, with its path. Children of closed lazy
    /// containers are left out; they are loaded with their panel.
    pub fn walk(&self) -> Vec<(NodePath, &Node)> {
        let mut out = Vec::new();
        for (i, node) in self.nodes().iter().enumerate() {
            walk_into(NodePath::from(i), node, &mut out);
        }
        out
    }

    /// The node at `path` and every node below it that renders with it.
    pub fn walk_from(&self, path: &NodePath) -> Vec<(NodePath, &Node)> {
        let mut out = Vec::new();
        if let Some(node) = self.node(path) {
            walk_into(path.clone(), node, &mut out);
        }
        out
    }
}

fn walk_into<'a>(path: NodePath, node: &'a Node, out: &mut Vec<(NodePath, &'a Node)>) {
    let children = node.eager_children();
    out.push((path.clone(), node));
    for (i, child) in children.iter().enumerate() {
        walk_into(NodePath(format!("{}.{i}", path.0)), child, out);
    }
}

/// Writes a node with its wrapper and path. Grids use their preloaded first page when there is one.
pub fn render_node(out: &mut String, r: &Render, path: &str, node: &Node) {
    out.push_str(&format!(r#"<div class="z-node" data-zeyn-node="{path}">"#));
    let r = Render { path, ..*r };
    match (node.grid(), r.loaded.and_then(|l| l.get(path))) {
        (Some(grid), Some(page)) => grid.render_loaded(out, &r, page),
        _ => node.render(out, &r),
    }
    out.push_str("</div>");
}

/// Writes child `index` of the container being rendered. Containers call this for each child.
pub fn render_child(out: &mut String, r: &Render, index: usize, node: &Node) {
    let path = if r.path.is_empty() { index.to_string() } else { format!("{}.{index}", r.path) };
    render_node(out, r, &path, node);
}

/// A DOM id unique on the page for the node being rendered: `{scope}-{what}-{path}`.
pub(crate) fn dom_id(r: &Render, what: &str) -> String {
    format!("{}-{what}-{}", r.scope, r.path.replace('.', "-"))
}

/// Data a component fetches from the server through an action, such as lookup options or tree
/// children. The server finds it by node path and key, checks the screen's access, the feature's
/// guards and the action's permission, then runs it.
pub trait DataSource: Send + Sync {
    fn action(&self) -> ActionMeta;

    /// Runs the action with the client's input. The input is untrusted: it is read as the
    /// action's input type, and anything that does not fit is refused.
    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>>;
}

/// A data source backed by an action that returns `Result<T>`.
pub(crate) struct ActionSource<A>(PhantomData<fn() -> A>);

impl<A> ActionSource<A> {
    pub(crate) fn new() -> Self {
        ActionSource(PhantomData)
    }
}

impl<A, T> DataSource for ActionSource<A>
where
    A: Action<Output = Result<T>>,
    A::Input: DeserializeOwned,
    T: Serialize + Send + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let input: A::Input = serde_json::from_value(input).map_err(|_| crate::action::Error::NotFound)?;
            let output = A::call(ctx, input).await??;
            serde_json::to_value(output).map_err(|e| crate::action::Error::Internal(e.to_string()))
        })
    }
}
