//! [`Indicator`]: a count or a dot on the corner of another component.

use super::layout::into_node;
use super::render_child;
use crate::grid::Tone;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Indicator);

/// The largest count an [`Indicator`] writes out; a larger one shows as "99+".
pub const MAX_INDICATOR_COUNT: u32 = 99;

/// What an [`Indicator`] shows on the corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    None,
    Dot,
    Count(u32),
}

/// A count or a dot on the corner of another component: unread messages on an avatar, something
/// new on a link. `.count(n)` shows the number, "99+" above [`MAX_INDICATOR_COUNT`], and nothing
/// at zero; `.dot()` shows a dot. The tone is `Danger` unless set.
///
/// The mark itself is hidden from screen readers, because a bare "3" after a name says little.
/// `.label(text)` gives the words they hear instead, such as "3 unread messages"; give one
/// whenever the mark carries meaning the component under it does not already state. Without a
/// label a count is read as its number and a dot is not read.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Indicator;
///
/// let inbox = Indicator::on(Avatar::new("Ada Lovelace"))
///     .count(3)
///     .label("3 unread messages");
/// let online = Indicator::on(Avatar::new("Alan Turing"))
///     .dot()
///     .tone(Tone::Success)
///     .label("Online");
/// ```
pub struct Indicator {
    nodes: Vec<Node>,
    mark: Mark,
    tone: Tone,
    label: Option<Text>,
}

impl Indicator {
    /// An indicator on `node`. It shows nothing until `.count` or `.dot` is called.
    pub fn on(node: impl Into<Node>) -> Self {
        Indicator {
            nodes: vec![node.into()],
            mark: Mark::None,
            tone: Tone::Danger,
            label: None,
        }
    }

    /// Shows a number. Zero shows nothing.
    pub fn count(mut self, count: u32) -> Self {
        self.mark = Mark::Count(count);
        self
    }

    /// Shows a dot.
    pub fn dot(mut self) -> Self {
        self.mark = Mark::Dot;
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// What screen readers hear for the mark, such as "3 unread messages".
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The text of the mark, or `None` when nothing shows.
    fn shown(&self) -> Option<String> {
        match self.mark {
            Mark::None | Mark::Count(0) => None,
            Mark::Dot => Some(String::new()),
            Mark::Count(n) if n > MAX_INDICATOR_COUNT => Some(format!("{MAX_INDICATOR_COUNT}+")),
            Mark::Count(n) => Some(n.to_string()),
        }
    }
}

impl Component for Indicator {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(r#"<div class="z-indicator">"#);
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        if let Some(text) = self.shown() {
            let kind = if text.is_empty() { "dot" } else { "count" };
            out.push_str(&format!(
                r#"<span class="z-indicator-mark z-indicator-{kind} z-tone-{}" aria-hidden="true">{text}</span>"#,
                self.tone.code()
            ));
            let spoken = match &self.label {
                Some(label) => r.text(label),
                None => text,
            };
            if !spoken.is_empty() {
                out.push_str(&format!(r#"<span class="z-sr-only">{spoken}</span>"#));
            }
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        let (kind, count) = match self.mark {
            Mark::None => ("none", None),
            Mark::Dot => ("dot", None),
            Mark::Count(n) => ("count", Some(n)),
        };
        json!({
            "type": "indicator",
            "mark": kind,
            "count": count,
            "tone": self.tone,
            "label": self.label,
            "children": self.nodes.iter().map(Node::schema).collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
