//! Layout containers: `Row`, `Stack`, `Panel`, `Card`, `Tabs` and `Split`. Each holds other nodes,
//! so grids, forms and other containers can be placed inside.

use super::{dom_id, render_child};
use crate::i18n::Text;
use crate::render::{Render, slug};
use crate::screen::{Component, Node, Toolbar};
use serde_json::{Value as Json, json};

/// Implements `From<T> for Node` for components of this library.
macro_rules! into_node {
    ($($t:ty),* $(,)?) => {
        $(impl From<$t> for Node {
            fn from(c: $t) -> Self {
                Node::new(c)
            }
        })*
    };
}
pub(crate) use into_node;

into_node!(Row, Stack, Panel, Card, Tabs, Split);

fn children_schema(nodes: &[Node]) -> Vec<Json> {
    nodes.iter().map(Node::schema).collect()
}

/// Columns side by side. Each column takes a share of the width by its weight; on narrow screens
/// the columns stack.
///
/// ```ignore
/// Row::new().col(customer_form).weight(2).col(order_totals)
/// ```
#[derive(Default)]
pub struct Row {
    nodes: Vec<Node>,
    weights: Vec<u8>,
    compact: bool,
}

impl Row {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a column with weight 1.
    pub fn col(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self.weights.push(1);
        self
    }

    /// Columns as wide as their content, wrapping when there is no room: for badges and buttons.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Weight of the last column, 1 to 12. A column of weight 2 is twice as wide as one of weight 1.
    pub fn weight(mut self, weight: u8) -> Self {
        if let Some(w) = self.weights.last_mut() {
            *w = weight.clamp(1, 12);
        }
        self
    }
}

impl Component for Row {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(if self.compact {
            r#"<div class="z-row z-row-compact">"#
        } else {
            r#"<div class="z-row">"#
        });
        for (i, node) in self.nodes.iter().enumerate() {
            out.push_str(&format!(
                r#"<div class="z-col" style="--z-flex:{}">"#,
                self.weights[i]
            ));
            render_child(out, r, i, node);
            out.push_str("</div>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        let columns: Vec<Json> = self
            .nodes
            .iter()
            .zip(&self.weights)
            .map(|(n, w)| json!({ "weight": w, "node": n.schema() }))
            .collect();
        json!({ "type": "row", "columns": columns })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// Nodes one under the other with a gap.
#[derive(Default)]
pub struct Stack {
    nodes: Vec<Node>,
    gap: u8,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            gap: 3,
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }

    /// Gap in spacing units (4 px at regular density), 0 to 12. The default is 3.
    pub fn gap(mut self, units: u8) -> Self {
        self.gap = units.min(12);
        self
    }
}

impl Component for Stack {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-stack" style="--z-stack-gap:{}">"#,
            self.gap
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "stack", "gap": self.gap, "children": children_schema(&self.nodes) })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// A titled section with an optional toolbar. It can collapse when `collapsible` is set.
pub struct Panel {
    title: Text,
    nodes: Vec<Node>,
    toolbar: Option<Toolbar>,
    collapsible: bool,
    collapsed: bool,
}

impl Panel {
    pub fn new(title: impl Into<Text>) -> Self {
        Self {
            title: title.into(),
            nodes: Vec::new(),
            toolbar: None,
            collapsible: false,
            collapsed: false,
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }

    /// Lets the user collapse the panel with a button in its title.
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }

    /// Starts collapsed. Implies `collapsible`.
    pub fn collapsed(mut self) -> Self {
        self.collapsible = true;
        self.collapsed = true;
        self
    }

    pub fn toolbar<F, R>(mut self, build: F) -> Self
    where
        F: FnOnce(Toolbar) -> R,
        R: Into<Toolbar>,
    {
        self.toolbar = Some(build(Toolbar::default()).into());
        self
    }
}

impl Component for Panel {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "panel");
        let title = r.text(&self.title);
        out.push_str(&format!(
            r#"<section class="z-panel" aria-labelledby="{id}-title"><header class="z-panel-head">"#
        ));
        if self.collapsible {
            out.push_str(&format!(
                r#"<h2 class="z-panel-title" id="{id}-title"><button type="button" class="z-panel-toggle" data-zeyn-toggle aria-expanded="{}" aria-controls="{id}-body" data-testid="{}.panel.{}"><span class="z-chevron" aria-hidden="true"></span>{title}</button></h2>"#,
                !self.collapsed,
                r.scope,
                slug(&self.title.english())
            ));
        } else {
            out.push_str(&format!(
                r#"<h2 class="z-panel-title" id="{id}-title">{title}</h2>"#
            ));
        }
        if let Some(t) = &self.toolbar {
            t.render(out, r, &title);
        }
        let hidden = if self.collapsed { " hidden" } else { "" };
        out.push_str(&format!(
            r#"</header><div class="z-panel-body" id="{id}-body"{hidden}>"#
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div></section>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "panel",
            "title": self.title,
            "collapsible": self.collapsible,
            "collapsed": self.collapsed,
            "toolbar": self.toolbar.as_ref().map(Toolbar::schema),
            "children": children_schema(&self.nodes),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// A box that groups related content: an optional title with one action at its end
/// (`.action(..)`, such as a "View all" link or a menu), the content, and an optional footer
/// (`.footer(..)`). A grid or list that is the card's content runs edge to edge without a second
/// border, and a card inside a card loses its own box.
///
/// ```
/// use zeyn::prelude::*;
///
/// let card = Card::new()
///     .title("Latest orders")
///     .action(Badge::new("12").count())
///     .add(Paragraph::new("Orders of today."))
///     .footer(Paragraph::new("Updated a minute ago."));
/// ```
#[derive(Default)]
pub struct Card {
    title: Option<Text>,
    nodes: Vec<Node>,
    /// Index in `nodes` of the title row's action.
    action: Option<usize>,
    /// Index in `nodes` of the footer.
    footer: Option<usize>,
}

impl Card {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// One action at the end of the title row, such as a link button or a menu button. Needs a
    /// title.
    pub fn action(mut self, node: impl Into<Node>) -> Self {
        self.action = Some(self.nodes.len());
        self.nodes.push(node.into());
        self
    }

    /// A footer under a divider, such as a "View all orders" link.
    pub fn footer(mut self, node: impl Into<Node>) -> Self {
        self.footer = Some(self.nodes.len());
        self.nodes.push(node.into());
        self
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }
}

impl Component for Card {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "card");
        if let Some(t) = &self.title {
            let title = format!(
                r#"<h3 class="z-card-title" id="{id}-title">{}</h3>"#,
                r.text(t)
            );
            out.push_str(&format!(
                r#"<article class="z-card" aria-labelledby="{id}-title">"#
            ));
            match self.action {
                Some(a) => {
                    out.push_str(r#"<header class="z-card-head">"#);
                    out.push_str(&title);
                    out.push_str(r#"<div class="z-card-action">"#);
                    render_child(out, r, a, &self.nodes[a]);
                    out.push_str("</div></header>");
                }
                None => out.push_str(&title),
            }
        } else {
            out.push_str(r#"<article class="z-card">"#);
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if Some(i) == self.footer || (Some(i) == self.action && self.title.is_some()) {
                continue;
            }
            render_child(out, r, i, node);
        }
        if let Some(f) = self.footer {
            out.push_str(r#"<footer class="z-card-foot">"#);
            render_child(out, r, f, &self.nodes[f]);
            out.push_str("</footer>");
        }
        out.push_str("</article>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "card",
            "title": self.title,
            "action": self.action,
            "footer": self.footer,
            "children": children_schema(&self.nodes),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// Tabs. Only the first tab is rendered with the page; the others load from the server the first
/// time they open. The open tab is kept in the URL hash, so reloads and shared links keep it.
///
/// Keyboard (ARIA tabs pattern): Left and Right move between tabs and open them, Home and End go
/// to the first and last tab, Tab moves into the open panel.
///
/// `.remember(KEY)` keeps the tab the user opened under a choice key (see
/// [`crate::ClientPref::choice`]), so the same tabs open on it on later pages; a tab named in the
/// URL hash wins.
///
/// ```
/// use std::time::Duration;
/// use zeyn::prelude::*;
/// use zeyn::{ClientPref, Choice, Purpose};
///
/// pub const ORDER_TAB: ClientPref<Choice> =
///     ClientPref::choice("order-tab", Duration::from_secs(30 * 86_400), Purpose::Preference);
///
/// let tabs = Tabs::new("Order")
///     .tab("Lines", Paragraph::new("..."))
///     .tab("History", Paragraph::new("..."))
///     .remember(ORDER_TAB);
/// ```
pub struct Tabs {
    label: Text,
    titles: Vec<Text>,
    nodes: Vec<Node>,
    remember: Option<crate::prefs::ClientPref<crate::prefs::Choice>>,
}

impl Tabs {
    /// Tabs with an accessible name for the tab list, such as "Order details".
    pub fn new(label: impl Into<Text>) -> Self {
        Self {
            label: label.into(),
            titles: Vec::new(),
            nodes: Vec::new(),
            remember: None,
        }
    }

    pub fn tab(mut self, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.titles.push(title.into());
        self.nodes.push(node.into());
        self
    }

    /// Keeps the opened tab under a choice key registered with `App::client_prefs`.
    pub fn remember(mut self, key: crate::prefs::ClientPref<crate::prefs::Choice>) -> Self {
        self.remember = Some(key);
        self
    }
}

impl Component for Tabs {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "tabs");
        out.push_str(&format!(
            r#"<div class="z-tabs" data-zeyn-tabs="{id}"{} data-zeyn-loading="{}" data-zeyn-failed="{}"><div class="z-tablist" role="tablist" aria-label="{}">"#,
            self.remember
                .map(|k| k.attrs(r))
                .unwrap_or_default(),
            r.text(&Text::key("zeyn.ui.loading")),
            r.text(&Text::key("zeyn.ui.load_failed")),
            r.text(&self.label)
        ));
        for (i, title) in self.titles.iter().enumerate() {
            let selected = i == 0;
            out.push_str(&format!(
                r#"<button type="button" role="tab" class="z-tab" id="{id}-tab-{i}" aria-selected="{selected}" aria-controls="{id}-panel-{i}" tabindex="{}" data-zeyn-tab="{}" data-testid="{}.tab.{}">{}</button>"#,
                if selected { 0 } else { -1 },
                slug(&title.english()),
                r.scope,
                slug(&title.english()),
                r.text(title)
            ));
        }
        out.push_str("</div>");
        for (i, node) in self.nodes.iter().enumerate() {
            let path = if r.path.is_empty() {
                i.to_string()
            } else {
                format!("{}.{i}", r.path)
            };
            if i == 0 {
                out.push_str(&format!(r#"<div class="z-tabpanel" role="tabpanel" id="{id}-panel-{i}" aria-labelledby="{id}-tab-{i}" tabindex="0">"#));
                render_child(out, r, i, node);
            } else {
                out.push_str(&format!(
                    r#"<div class="z-tabpanel" role="tabpanel" id="{id}-panel-{i}" aria-labelledby="{id}-tab-{i}" tabindex="0" hidden data-zeyn-lazy="{path}">"#
                ));
            }
            out.push_str("</div>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        let tabs: Vec<Json> = self
            .titles
            .iter()
            .zip(&self.nodes)
            .map(|(t, n)| json!({ "title": t, "node": n.schema() }))
            .collect();
        json!({ "type": "tabs", "label": self.label, "tabs": tabs, "remember": self.remember.map(|k| k.info()) })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }

    fn eager_children(&self) -> &[Node] {
        &self.nodes[..self.nodes.len().min(1)]
    }
}

/// Two panes with a divider the user can drag, or move with the arrow keys when it has focus.
/// The panes stack on narrow screens.
pub struct Split {
    nodes: Vec<Node>,
    ratio: u8,
    vertical: bool,
}

impl Split {
    /// Two panes side by side; the first takes 40 percent.
    pub fn new(first: impl Into<Node>, second: impl Into<Node>) -> Self {
        Self {
            nodes: vec![first.into(), second.into()],
            ratio: 40,
            vertical: false,
        }
    }

    /// Share of the first pane in percent, 10 to 90.
    pub fn ratio(mut self, percent: u8) -> Self {
        self.ratio = percent.clamp(10, 90);
        self
    }

    /// Puts the panes one above the other.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
}

impl Component for Split {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "split");
        let (class, orientation) = if self.vertical {
            ("z-split z-split-v", "horizontal")
        } else {
            ("z-split", "vertical")
        };
        out.push_str(&format!(
            r#"<div class="{class}" data-zeyn-split style="--z-split:{}%">"#,
            self.ratio
        ));
        out.push_str(&format!(r#"<div class="z-split-pane" id="{id}-first">"#));
        render_child(out, r, 0, &self.nodes[0]);
        out.push_str(&format!(
            r#"</div><div class="z-split-bar" role="separator" tabindex="0" aria-orientation="{orientation}" aria-controls="{id}-first" aria-valuenow="{}" aria-valuemin="10" aria-valuemax="90" aria-label="{}" data-testid="{}.split"></div><div class="z-split-pane">"#,
            self.ratio,
            r.text(&Text::key("zeyn.ui.resize")),
            r.scope
        ));
        render_child(out, r, 1, &self.nodes[1]);
        out.push_str("</div></div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "split", "ratio": self.ratio, "vertical": self.vertical, "children": children_schema(&self.nodes) })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
