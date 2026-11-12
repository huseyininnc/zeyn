//! [`Accordion`]: sections under titles that open and close, one under the other.

use super::layout::into_node;
use super::{dom_id, render_child};
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Accordion);

/// Most sections an [`Accordion`] holds; sections added after that are left out.
pub const MAX_ACCORDION_SECTIONS: usize = 50;

/// Sections under titles, one under the other, each opened and closed by its title: the answers
/// of a help page, the parts of a long record. Every section is a native disclosure (`<details>`
/// and `<summary>`), so the titles are buttons for keyboards and screen readers, a closed
/// section stays out of the reading order, and the browser's find in page opens the section that
/// holds a match. No browser script is involved.
///
/// Sections start closed; `.open()` after a section opens it. `.single()` keeps one section open
/// at a time: the sections share a `name`, and the browser closes the open one when another
/// opens. In that mode only the first section marked open starts open.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Accordion;
///
/// let help = Accordion::new()
///     .section("How do I reset my password?", Paragraph::new("Use the link on the sign in page."))
///     .open()
///     .section("Where are my invoices?", Paragraph::new("Under Billing, then Invoices."))
///     .single();
/// ```
#[derive(Default)]
pub struct Accordion {
    titles: Vec<Text>,
    nodes: Vec<Node>,
    open: Vec<bool>,
    single: bool,
}

impl Accordion {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a closed section with its title and content. At most [`MAX_ACCORDION_SECTIONS`].
    pub fn section(mut self, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        if self.nodes.len() < MAX_ACCORDION_SECTIONS {
            self.titles.push(title.into());
            self.nodes.push(node.into());
            self.open.push(false);
        }
        self
    }

    /// Opens the section added last.
    pub fn open(mut self) -> Self {
        if let Some(last) = self.open.last_mut() {
            *last = true;
        }
        self
    }

    /// Keeps one section open at a time.
    pub fn single(mut self) -> Self {
        self.single = true;
        self
    }

    /// Whether section `index` starts open: as marked, and only the first marked one when a
    /// single section may be open.
    fn starts_open(&self, index: usize) -> bool {
        let marked = self
            .open
            .get(index)
            .copied()
            .unwrap_or(false);
        let first = self.open.iter().position(|o| *o);
        marked && (!self.single || first == Some(index))
    }
}

impl Component for Accordion {
    fn render(&self, out: &mut String, r: &Render) {
        let group = if self.single {
            format!(r#" name="{}""#, dom_id(r, "accordion"))
        } else {
            String::new()
        };
        out.push_str(r#"<div class="z-accordion">"#);
        for (i, (title, node)) in self
            .titles
            .iter()
            .zip(&self.nodes)
            .enumerate()
        {
            out.push_str(&format!(
                r#"<details class="z-accordion-item"{group}{}><summary class="z-accordion-title">{}</summary><div class="z-accordion-body">"#,
                if self.starts_open(i) { " open" } else { "" },
                r.text(title)
            ));
            render_child(out, r, i, node);
            out.push_str("</div></details>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        let sections: Vec<Json> = self
            .titles
            .iter()
            .zip(&self.nodes)
            .enumerate()
            .map(|(i, (t, n))| json!({ "title": t, "open": self.starts_open(i), "node": n.schema() }))
            .collect();
        json!({ "type": "accordion", "single": self.single, "sections": sections })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
