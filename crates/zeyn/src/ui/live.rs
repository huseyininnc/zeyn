//! `Live`: nodes that refresh in place when the server notifies a topic (feature `realtime`).

use super::{layout::into_node, render_child};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Live);

/// Wraps nodes that refresh when [`Socket::notify`](crate::realtime::Socket::notify) names `topic`:
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

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    #[track_caller]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(Node::placed(node));
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

/// What a node does when an event of the live socket arrives (see [`OnEvents::on_event`]).
#[cfg(feature = "realtime")]
pub enum OnEvent {
    /// The node refreshes in place: grids reload their current page, other nodes are rendered
    /// again, the same as `.listens` on a topic named after the event.
    Refresh,
    /// The page shows a toast with this text.
    Toast(crate::i18n::Text),
}

/// `.on_event::<E>(OnEvent::Refresh)` on every component: the node reacts when the live socket
/// delivers event `E` to this user. Only events the user's permission allows ever arrive. Call it
/// last, after the component's own builder methods.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::realtime::Event;
/// use zeyn::ui::{OnEvent, OnEvents};
/// #[derive(serde::Serialize, Event)]
/// #[event(name = "orders.shipped")]
/// struct Shipped { id: u32 }
/// let node = Alert::info("Orders").on_event::<Shipped>(OnEvent::Refresh);
/// ```
#[cfg(feature = "realtime")]
pub trait OnEvents: Into<Node> + Sized {
    fn on_event<E: crate::realtime::Event>(self, on: OnEvent) -> EventNode {
        let node: Node = match on {
            OnEvent::Refresh => super::Listens::listens(self, E::NAME).into(),
            OnEvent::Toast(_) => self.into(),
        };
        EventNode {
            name: E::NAME,
            toast: match on {
                OnEvent::Toast(t) => Some(t),
                OnEvent::Refresh => None,
            },
            node: vec![node],
        }
    }
}

#[cfg(feature = "realtime")]
impl<T: Into<Node>> OnEvents for T {}

/// A node that reacts to a live socket event; made by [`OnEvents::on_event`].
#[cfg(feature = "realtime")]
pub struct EventNode {
    name: &'static str,
    toast: Option<crate::i18n::Text>,
    node: Vec<Node>,
}

#[cfg(feature = "realtime")]
into_node!(EventNode);

#[cfg(feature = "realtime")]
impl Component for EventNode {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-on" data-zeyn-on="{}""#,
            escape(self.name)
        ));
        if let Some(text) = &self.toast {
            out.push_str(&format!(r#" data-zeyn-on-toast="{}""#, r.text(text)));
        }
        out.push('>');
        for (i, node) in self.node.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "on_event", "event": self.name, "toast": self.toast.is_some(), "node": self.node.first().map(Node::schema) })
    }

    fn children(&self) -> &[Node] {
        &self.node
    }
}
