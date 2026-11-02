//! Tooltips, popovers and the busy overlay.
//!
//! - A tooltip (APG tooltip pattern) is a short description of the element it belongs to,
//!   referenced with `aria-describedby`, so screen readers hear it with the element. It shows on
//!   hover and on focus, stays while the pointer is over it, hides with Escape without moving
//!   focus, and shows on touch after a long press. It never carries essential information: what a
//!   user must know goes in the label or the page.
//! - A [`Popover`] is a non-modal dialog that opens next to its button with small details.
//! - [`Busy`] covers its content with a spinner while a request inside it runs, after a short
//!   delay so fast requests do not flash.

use super::{dom_id, render_child};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

/// Most characters of a tooltip; longer help belongs in a popover or on the page.
pub const MAX_TIP: usize = 200;

/// The attributes of an element that has a tooltip and the tooltip itself, which the runtime
/// places next to it. The text is escaped and cut at [`MAX_TIP`] characters.
pub(crate) fn tip(r: &Render, id: &str, text: &Text) -> (String, String) {
    let text: String = r.i18n.text(r.locale, text).chars().take(MAX_TIP).collect();
    (
        format!(r#" aria-describedby="{id}" data-zeyn-tip="{id}""#),
        format!(r#"<span role="tooltip" class="z-tip" id="{id}" hidden>{}</span>"#, escape(&text)),
    )
}

/// A button that opens a non-modal dialog with details next to it: the APG dialog pattern
/// without `aria-modal`. Focus moves into the popover when it opens; Escape, the close button or
/// a click outside close it and focus returns to the button. Tab may leave it, which closes it.
///
/// ```ignore
/// Popover::new(t!("orders.shipping")).title(t!("orders.shipping_rules"))
///     .add(DescriptionList::of(&rules).item(Rules::cutoff).item(Rules::carrier))
/// ```
pub struct Popover {
    label: Text,
    title: Option<Text>,
    nodes: Vec<Node>,
}

impl Popover {
    pub fn new(label: impl Into<Text>) -> Self {
        Self { label: label.into(), title: None, nodes: Vec::new() }
    }

    /// Heading of the popover and its accessible name; the button label without it.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }
}

impl Component for Popover {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "popover");
        let label = r.text(&self.label);
        let title = self.title.as_ref().map_or_else(|| label.clone(), |t| r.text(t));
        out.push_str(&format!(
            r#"<div class="z-popover-wrap" data-zeyn-popover><button type="button" class="z-button z-popover-button" id="{id}-button" aria-haspopup="dialog" aria-expanded="false" aria-controls="{id}" data-testid="{}.popover.{}">{label}</button><div class="z-popover" id="{id}" role="dialog" aria-labelledby="{id}-title" tabindex="-1" hidden><div class="z-popover-head"><h2 class="z-popover-title" id="{id}-title">{title}</h2><button type="button" class="z-icon-button z-popover-close" data-zeyn-popover-close aria-label="{}"><span aria-hidden="true">&#215;</span></button></div><div class="z-popover-body">"#,
            r.scope,
            crate::render::slug(&self.label.english()),
            r.text(&Text::key("zeyn.ui.popover.close"))
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div></div></div>");
    }

    fn schema(&self) -> Json {
        let children: Vec<Json> = self.nodes.iter().map(Node::schema).collect();
        json!({ "type": "popover", "label": self.label, "title": self.title, "children": children })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// Default delay before the busy overlay shows, in milliseconds.
pub const BUSY_DELAY_MS: u32 = 400;

/// A container that shows a busy overlay over its content while a request inside it runs: a
/// form saving, a grid loading, an action button running. The container gets `aria-busy` at
/// once; the spinner and the text ("Working") show only after [`BUSY_DELAY_MS`] (or the delay
/// given), so requests that answer quickly do not flash. Once shown, the overlay stays at least
/// 300 ms. The overlay blocks the pointer but not the keyboard, and focus stays where it is.
///
/// ```ignore
/// Busy::new().add(Form::of(order).field(Order::code).submit(t!("save"), save_order))
/// ```
pub struct Busy {
    delay_ms: u32,
    nodes: Vec<Node>,
}

impl Default for Busy {
    fn default() -> Self {
        Self { delay_ms: BUSY_DELAY_MS, nodes: Vec::new() }
    }
}

impl Busy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Milliseconds before the overlay shows, at most 5000.
    pub fn delay_ms(mut self, ms: u32) -> Self {
        self.delay_ms = ms.min(5000);
        self
    }

    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }
}

impl Component for Busy {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<div class="z-busy" data-zeyn-busy="{}">"#, self.delay_ms));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str(&format!(
            r#"<div class="z-busy-overlay" hidden><span class="z-spinner" aria-hidden="true"></span><span class="z-busy-text" role="status" data-text="{}"></span></div></div>"#,
            r.text(&Text::key("zeyn.ui.busy"))
        ));
    }

    fn schema(&self) -> Json {
        let children: Vec<Json> = self.nodes.iter().map(Node::schema).collect();
        json!({ "type": "busy", "delay_ms": self.delay_ms, "children": children })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

super::layout::into_node!(Popover, Busy);
