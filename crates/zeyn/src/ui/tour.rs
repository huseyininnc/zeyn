//! [`Tour`]: a button that walks the user through the [`TourStop`]s of a page.

use super::layout::into_node;
use super::{dom_id, render_child};
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Tour, TourStop);

/// The most stops a tour visits; the browser leaves out the ones after it.
pub const MAX_TOUR_STOPS: usize = 30;

/// A button that starts a guided tour of the page: a small dialog visits every [`TourStop`] of
/// its screen in the order of the page, with Back, Next and Close. Use one tour per screen. The `tour.js` module shows the button, so
/// without script there is no tour and the page reads as it is.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Tour, TourStop};
///
/// let start = Tour::new("Show me around");
/// let stop = TourStop::on(Badge::new("Draft"))
///     .title("Status")
///     .body("An order stays a draft until it is sent.");
/// ```
pub struct Tour {
    label: Text,
}

impl Tour {
    /// A tour whose button reads `label`.
    pub fn new(label: impl Into<Text>) -> Self {
        Tour {
            label: label.into(),
        }
    }
}

impl Component for Tour {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "tour");
        let progress = Text::key("zeyn.ui.steps.progress")
            .arg("n", "{n}")
            .arg("total", "{total}");
        let word = |key: &'static str| r.text(&Text::key(key));
        out.push_str(&format!(
            r#"<div class="z-tour" data-zeyn-tour><button type="button" class="z-button z-tour-start" data-testid="{}.tour" hidden>{}</button><dialog class="z-tour-card" aria-labelledby="{id}-title" aria-describedby="{id}-body"><div aria-live="polite"><p class="z-tour-count" data-progress="{}"></p><h2 class="z-tour-title" id="{id}-title"></h2><p class="z-tour-body" id="{id}-body"></p></div><div class="z-tour-actions"><button type="button" class="z-button z-tour-close">{}</button><button type="button" class="z-button z-tour-back">{}</button><button type="button" class="z-button z-primary z-tour-next" data-next="{}" data-finish="{}"></button></div></dialog></div>"#,
            r.scope,
            r.text(&self.label),
            r.text(&progress),
            word("zeyn.ui.tour.close"),
            word("zeyn.ui.steps.back"),
            word("zeyn.ui.steps.next"),
            word("zeyn.ui.tour.finish"),
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "tour", "label": self.label })
    }
}

/// A component a [`Tour`] stops at, with the title and the text the tour shows there. It draws
/// its component unchanged; a stop that is not visible is skipped.
pub struct TourStop {
    nodes: Vec<Node>,
    title: Text,
    body: Text,
}

impl TourStop {
    /// A stop at `node`.
    #[track_caller]
    pub fn on(node: impl Into<Node>) -> Self {
        TourStop {
            nodes: vec![Node::placed(node)],
            title: Text::from(""),
            body: Text::from(""),
        }
    }

    /// The heading of the stop.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = title.into();
        self
    }

    /// The sentence or two under the heading.
    pub fn body(mut self, body: impl Into<Text>) -> Self {
        self.body = body.into();
        self
    }
}

impl Component for TourStop {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-tourstop" data-tour-title="{}" data-tour-body="{}">"#,
            r.text(&self.title),
            r.text(&self.body)
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "tour_stop",
            "title": self.title,
            "body": self.body,
            "children": self.nodes.iter().map(Node::schema).collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
