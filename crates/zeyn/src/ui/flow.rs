//! [`FlowDiagram`]: the steps of a process as boxes joined by arrows.

use super::dom_id;
use super::layout::into_node;
use crate::grid::Tone;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(FlowDiagram);

/// The most steps a [`FlowDiagram`] draws; the ones after it are left out.
pub const MAX_FLOW_NODES: usize = 40;

/// The most arrows a [`FlowDiagram`] draws; the ones after it are left out.
pub const MAX_FLOW_EDGES: usize = 80;

/// The size of a box and the space between boxes, in pixels.
const BOX: (usize, usize) = (168, 56);
const GAP: (usize, usize) = (72, 24);

/// The room under the boxes for arrows that lead back to an earlier step.
const BACK_ROOM: usize = 56;

struct FlowNode {
    id: String,
    title: Text,
    tone: Option<Tone>,
}

struct FlowEdge {
    from: String,
    to: String,
    label: Option<Text>,
}

/// Where the boxes go: the column and row of each step, and which arrows lead back.
struct Layout {
    /// The top start corner of each step in pixels.
    at: Vec<(usize, usize)>,
    /// The arrows as indexes of steps, with whether one leads back to an earlier column.
    edges: Vec<(usize, usize, bool, usize)>,
    size: (usize, usize),
}

/// The steps of a process as boxes joined by arrows, drawn from the start side to the end side:
/// an order from cart to delivery, an approval with its branches. `.node(id, title)` adds a step,
/// `.tone(...)` colors the step added last, `.edge(from, to)` draws an arrow between two steps
/// and `.edge_with(from, to, label)` one with a word on it. Steps are placed in columns by how
/// far they are from the first steps; an arrow to an earlier step runs under the boxes.
///
/// The steps are an ordered list and each one names the steps it leads to for screen readers.
/// A wide diagram scrolls inside its own box.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::FlowDiagram;
///
/// let flow = FlowDiagram::new("Order process")
///     .node("cart", "Cart")
///     .node("pay", "Payment")
///     .node("ship", "Shipping")
///     .tone(Tone::Success)
///     .edge("cart", "pay")
///     .edge_with("pay", "ship", "Paid");
/// ```
pub struct FlowDiagram {
    label: Text,
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
}

impl FlowDiagram {
    /// A diagram named by `label`.
    pub fn new(label: impl Into<Text>) -> Self {
        FlowDiagram {
            label: label.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Adds a step. A second step with the same `id` is left out.
    pub fn node(mut self, id: impl Into<String>, title: impl Into<Text>) -> Self {
        let id = id.into();
        if self.nodes.len() < MAX_FLOW_NODES && self.nodes.iter().all(|n| n.id != id) {
            self.nodes.push(FlowNode {
                id,
                title: title.into(),
                tone: None,
            });
        }
        self
    }

    /// Colors the step added last.
    pub fn tone(mut self, tone: Tone) -> Self {
        if let Some(node) = self.nodes.last_mut() {
            node.tone = Some(tone);
        }
        self
    }

    /// An arrow from one step to another, by their ids. One that names an unknown step is left
    /// out.
    pub fn edge(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.push_edge(from.into(), to.into(), None)
    }

    /// An arrow with a word on it, such as "Yes" or "Paid".
    pub fn edge_with(
        self,
        from: impl Into<String>,
        to: impl Into<String>,
        label: impl Into<Text>,
    ) -> Self {
        self.push_edge(from.into(), to.into(), Some(label.into()))
    }

    fn push_edge(mut self, from: String, to: String, label: Option<Text>) -> Self {
        if self.edges.len() < MAX_FLOW_EDGES && from != to {
            self.edges
                .push(FlowEdge { from, to, label });
        }
        self
    }

    /// Marks the arrows that close a loop, walking from `at`.
    fn mark_back(at: usize, edges: &mut [(usize, usize, bool, usize)], state: &mut [u8]) {
        state[at] = 1;
        for i in 0..edges.len() {
            let (from, to, _, _) = edges[i];
            if from != at {
                continue;
            }
            match state[to] {
                0 => Self::mark_back(to, edges, state),
                1 => edges[i].2 = true,
                _ => {}
            }
        }
        state[at] = 2;
    }

    fn layout(&self) -> Layout {
        let n = self.nodes.len();
        let index = |id: &str| {
            self.nodes
                .iter()
                .position(|node| node.id == id)
        };
        let mut edges: Vec<(usize, usize, bool, usize)> = self
            .edges
            .iter()
            .enumerate()
            .filter_map(|(i, e)| Some((index(&e.from)?, index(&e.to)?, false, i)))
            .collect();
        let mut state = vec![0u8; n];
        for start in 0..n {
            if state[start] == 0 {
                Self::mark_back(start, &mut edges, &mut state);
            }
        }
        let mut column = vec![0usize; n];
        for _ in 0..n {
            for &(from, to, back, _) in &edges {
                if !back && column[to] <= column[from] {
                    column[to] = column[from] + 1;
                }
            }
        }
        let columns = column.iter().max().map_or(0, |c| c + 1);
        let mut row = vec![0usize; n];
        let mut tallest = 0;
        let mut counts = vec![0usize; columns];
        for (c, count) in counts.iter_mut().enumerate() {
            let mut members: Vec<usize> = (0..n)
                .filter(|&i| column[i] == c)
                .collect();
            let weight = |i: usize| {
                let before: Vec<usize> = edges
                    .iter()
                    .filter(|e| !e.2 && e.1 == i)
                    .map(|e| row[e.0])
                    .collect();
                match before.len() {
                    0 => i * 1000,
                    len => before.iter().sum::<usize>() * 1000 / len,
                }
            };
            members.sort_by_key(|&i| weight(i));
            for (r, &i) in members.iter().enumerate() {
                row[i] = r;
            }
            *count = members.len();
            tallest = tallest.max(members.len());
        }
        let step = (BOX.0 + GAP.0, BOX.1 + GAP.1);
        let at = (0..n)
            .map(|i| {
                let shift = (tallest - counts[column[i]]) * step.1 / 2;
                (column[i] * step.0, row[i] * step.1 + shift)
            })
            .collect();
        let back = edges.iter().any(|e| e.2);
        let size = (
            (columns * step.0).saturating_sub(GAP.0),
            (tallest * step.1).saturating_sub(GAP.1) + if back { BACK_ROOM } else { 0 },
        );
        Layout { at, edges, size }
    }
}

impl Component for FlowDiagram {
    fn render(&self, out: &mut String, r: &Render) {
        let layout = self.layout();
        let (width, height) = layout.size;
        let marker = dom_id(r, "flow");
        out.push_str(&format!(
            r#"<div class="z-flow" role="group" aria-label="{}" tabindex="0"><div class="z-flow-canvas" style="--z-flow-w:{width}px;--z-flow-h:{height}px"><svg class="z-flow-lines" viewBox="0 0 {width} {height}" width="{width}" height="{height}" aria-hidden="true" focusable="false"><defs><marker id="{marker}" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z"/></marker></defs>"#,
            r.text(&self.label)
        ));
        let mut labels = String::new();
        for &(from, to, back, edge) in &layout.edges {
            let (fx, fy) = layout.at[from];
            let (tx, ty) = layout.at[to];
            let (path, mid) = if back {
                let (sx, sy) = (fx + BOX.0 / 2, fy + BOX.1);
                let (ex, ey) = (tx + BOX.0 / 2, ty + BOX.1);
                let low = sy.max(ey) + BACK_ROOM - 8;
                (
                    format!("M{sx} {sy}C{sx} {low} {ex} {low} {ex} {ey}"),
                    ((sx + ex) / 2, sy.max(ey) + (BACK_ROOM - 8) * 3 / 4),
                )
            } else {
                let (sx, sy) = (fx + BOX.0, fy + BOX.1 / 2);
                let (ex, ey) = (tx, ty + BOX.1 / 2);
                let bend = (ex.saturating_sub(sx) / 2).max(GAP.0 / 2);
                (
                    format!(
                        "M{sx} {sy}C{} {sy} {} {ey} {ex} {ey}",
                        sx + bend,
                        ex.saturating_sub(bend)
                    ),
                    ((sx + ex) / 2, (sy + ey) / 2),
                )
            };
            out.push_str(&format!(
                r#"<path class="z-flow-line" d="{path}" marker-end="url(#{marker})"/>"#
            ));
            if let Some(label) = &self.edges[edge].label {
                labels.push_str(&format!(
                    r#"<span class="z-flow-label" style="--z-flow-x:{}px;--z-flow-y:{}px" aria-hidden="true">{}</span>"#,
                    mid.0,
                    mid.1,
                    r.text(label)
                ));
            }
        }
        out.push_str(r#"</svg><ol class="z-flow-nodes">"#);
        for (i, node) in self.nodes.iter().enumerate() {
            let (x, y) = layout.at[i];
            let tone = node
                .tone
                .map(|t| format!(" z-tone-{}", t.code()))
                .unwrap_or_default();
            out.push_str(&format!(
                r#"<li class="z-flow-node{tone}" style="--z-flow-x:{x}px;--z-flow-y:{y}px"><span class="z-flow-title">{}</span>"#,
                r.text(&node.title)
            ));
            let next: Vec<String> = layout
                .edges
                .iter()
                .filter(|e| e.0 == i)
                .map(|e| {
                    let title = r
                        .i18n
                        .text(r.locale, &self.nodes[e.1].title);
                    match &self.edges[e.3].label {
                        Some(label) => format!("{title} ({})", r.i18n.text(r.locale, label)),
                        None => title,
                    }
                })
                .collect();
            if !next.is_empty() {
                let text = Text::key("zeyn.ui.flow.next").arg("steps", next.join(", "));
                out.push_str(&format!(
                    r#"<span class="z-sr-only">{}</span>"#,
                    r.text(&text)
                ));
            }
            out.push_str("</li>");
        }
        out.push_str("</ol>");
        out.push_str(&labels);
        out.push_str("</div></div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "flow_diagram",
            "label": self.label,
            "nodes": self.nodes.iter().map(|n| json!({ "id": n.id, "title": n.title, "tone": n.tone })).collect::<Vec<_>>(),
            "edges": self.edges.iter().map(|e| json!({ "from": e.from, "to": e.to, "label": e.label })).collect::<Vec<_>>(),
        })
    }
}
