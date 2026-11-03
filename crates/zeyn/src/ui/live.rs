//! `Live`: nodes that refresh in place when the server notifies a topic (feature `realtime`).

use super::{layout::into_node, render_child};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Live);

/// Wraps nodes that refresh when [`Hub::notify`](crate::realtime::Hub::notify) names `topic`:
/// grids inside reload their current page in place; without a grid the screen is rendered again
/// in place. The topic must be declared with `Realtime::topic`, and only users with its
/// permission are subscribed.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Live;
/// use zeyn::{Component, I18n, Render};
///
/// let live = Live::new("orders").add(Alert::info("Orders"));
/// let mut html = String::new();
/// live.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.starts_with(r#"<div class="z-live" data-zeyn-live="orders">"#));
/// ```
pub struct Live {
    topic: &'static str,
    nodes: Vec<Node>,
}

impl Live {
    pub fn new(topic: &'static str) -> Self {
        Live {
            topic,
            nodes: Vec::new(),
        }
    }

    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }
}

impl Component for Live {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-live" data-zeyn-live="{}">"#,
            escape(self.topic)
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "live", "topic": self.topic, "nodes": self.nodes.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
