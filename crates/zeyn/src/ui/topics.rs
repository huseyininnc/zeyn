//! Topics: when one part of a screen changes, the parts that show it refresh in place.
//!
//! A node marked with `.listens("sales")` refreshes when an action asks for it
//! ([`crate::Effect::refresh`]), when a filter control that `.drives("sales")` changes, or when
//! the realtime hub notifies `sales` (feature `realtime`). Only topic names travel; every refresh
//! goes through the normal endpoints with every permission check.

use super::{layout::into_node, render_child};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Listen);

/// Most topics one node listens to or drives.
pub const MAX_TOPICS: usize = 8;

/// One topic name or several, for `.listens`, `.drives` and [`crate::Effect::refresh`]:
/// `"sales"`, `["sales", "stock"]` or a `&'static [&'static str]`. Topics are names written in
/// the code, so `zeyn check` can match listeners and senders; a name built at run time does not
/// compile:
///
/// ```compile_fail
/// use zeyn::prelude::*;
/// let topic = format!("sales.{}", 7);
/// let kpi = Kpi::new("Total", 1.0).listens(topic);
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a topic name written in the code",
    label = "expected `\"topic\"`, `[\"a\", \"b\"]` or a `&'static [&'static str]`",
    note = "topics are fixed names such as `\"sales\"`; put what varies (a region, a record id) in the screen params, which listeners read when they refresh"
)]
pub trait Topics {
    fn topics(self) -> Vec<&'static str>;
}

impl Topics for &'static str {
    fn topics(self) -> Vec<&'static str> {
        vec![self]
    }
}

impl<const N: usize> Topics for [&'static str; N] {
    fn topics(self) -> Vec<&'static str> {
        self.to_vec()
    }
}

impl Topics for &'static [&'static str] {
    fn topics(self) -> Vec<&'static str> {
        self.to_vec()
    }
}

/// The topics a builder keeps: at most [`MAX_TOPICS`], each once. Names are checked by
/// `App::check_page` ([`valid_topic`]).
pub(crate) fn topic_list(topics: impl Topics) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for t in topics.topics() {
        if out.len() < MAX_TOPICS && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// Whether a topic name can be used: 1 to 64 letters, digits, `-`, `_` or `.`, the same names
/// `Realtime::topic` takes.
///
/// ```
/// use zeyn::ui::valid_topic;
/// assert!(valid_topic("sales.region"));
/// assert!(!valid_topic("a,b"));
/// assert!(!valid_topic(""));
/// ```
pub fn valid_topic(topic: &str) -> bool {
    (1..=64).contains(&topic.len())
        && topic
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A data attribute holding topics, comma separated, or nothing when there are none.
pub(crate) fn topics_attr(name: &str, topics: &[&'static str]) -> String {
    if topics.is_empty() {
        return String::new();
    }
    format!(r#" {name}="{}""#, escape(&topics.join(",")))
}

/// A node that refreshes in place when one of its topics is refreshed, driven or notified. Made
/// with [`Listens::listens`] on any component.
///
/// A listener that holds a DataGrid reloads its grids through `POST /_zeyn/grid`: the current
/// page, sort and filters after [`crate::Effect::refresh`], the first page after a filter that
/// drives the topic changed. Any other listener (a chart, a KPI, a list, a panel) is rendered
/// again through `POST /_zeyn/nodes`, one request for every listener of the screen. Focus and
/// open `<details>` stay where they were when the new content has the same element (by id, test
/// id or row key).
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::{Component, I18n, Render};
///
/// let kpi = Kpi::new("Revenue", 184_250.0).listens(["sales", "stock"]);
/// let mut html = String::new();
/// kpi.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.starts_with(r#"<div class="z-listen" data-zeyn-listens="sales,stock">"#));
/// ```
pub struct Listen {
    topics: Vec<&'static str>,
    node: Vec<Node>,
}

impl Component for Listen {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-listen"{}>"#,
            topics_attr("data-zeyn-listens", &self.topics)
        ));
        for (i, node) in self.node.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "listen", "topics": self.topics, "node": self.node.first().map(Node::schema) })
    }

    fn children(&self) -> &[Node] {
        &self.node
    }

    fn page_check(&self, check: &mut PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
    }
}

/// `.listens(topics)` on every component: the node refreshes in place when one of the topics is
/// refreshed ([`crate::Effect::refresh`]), driven by a filter control (`.drives`) or notified by
/// the realtime hub. Call it last, after the component's own builder methods; the node keeps its
/// own endpoints one level down (`p.0` for a listener at `p`).
///
/// ```
/// use zeyn::prelude::*;
/// let chart = Chart::bar("Sales by month")
///     .labels(["Jan", "Feb"])
///     .series("2026", [120.0, 140.0])
///     .listens("sales");
/// ```
pub trait Listens: Into<Node> + Sized {
    fn listens(self, topics: impl Topics) -> Listen {
        Listen {
            topics: topic_list(topics),
            node: vec![self.into()],
        }
    }
}

impl<T: Into<Node>> Listens for T {}

/// What `App::check_page` learns from a node: the topics it listens to and drives, and problems
/// of its own configuration. Components that take part in topics, or have rules the compiler
/// cannot check, report them from [`Component::page_check`].
#[derive(Debug, Default)]
pub struct PageCheck {
    pub(crate) path: String,
    pub(crate) listens: Vec<&'static str>,
    pub(crate) drives: Vec<(String, &'static str)>,
    pub(crate) problems: Vec<String>,
}

impl PageCheck {
    /// The node listens to `topic`.
    pub fn listens(&mut self, topic: &'static str) {
        self.topic(topic);
        self.listens.push(topic);
    }

    /// The node drives `topic`: changing it refreshes the listeners of the topic.
    pub fn drives(&mut self, topic: &'static str) {
        self.topic(topic);
        self.drives
            .push((self.path.clone(), topic));
    }

    /// A problem of the node's configuration, written for whoever fixes it. The node's path is
    /// put in front.
    pub fn problem(&mut self, message: impl AsRef<str>) {
        self.problems
            .push(format!("node {}: {}", self.path, message.as_ref()));
    }

    fn topic(&mut self, topic: &str) {
        if !valid_topic(topic) {
            self.problem(format!(
                "topic `{topic}` is not a valid name; use 1 to 64 letters, digits, `-`, `_` or `.`"
            ));
        }
    }

    /// Problems of the topics of the page: a topic driven on the page that nothing on the page
    /// listens to.
    pub(crate) fn finish(mut self) -> Vec<String> {
        for (path, topic) in &self.drives {
            if !self.listens.contains(topic) {
                self.problems.push(format!(
                    "node {path} drives topic `{topic}`, but nothing on this page listens to it; add `.listens(\"{topic}\")` to the nodes that should refresh"
                ));
            }
        }
        self.problems
    }
}
