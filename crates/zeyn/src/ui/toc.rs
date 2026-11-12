//! [`Toc`]: links to the headings of the page being shown.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Toc);

/// Most entries a [`Toc`] holds, sub entries included; entries added after that are left out.
pub const MAX_TOC_ITEMS: usize = 100;

struct Entry {
    title: Text,
    id: String,
    sub: bool,
}

/// An anchor as [`super::Heading::id`] keeps it: letters, digits, `-` and `_`, at most 64.
fn anchor(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect()
}

/// A table of contents: links to the headings of the page being shown, for long pages such as
/// an article or a policy. Each entry names a heading and the anchor it was given with
/// `Heading::id`; `.sub` adds an entry one level under the entry before it. The anchors are
/// cleaned as `Heading::id` cleans them, so the same string works in both places, and an entry
/// whose anchor comes out empty is left out.
///
/// The links are plain links to `#anchor` in a `nav` named by the label, so they work without
/// script, the browser scrolls to the heading and moves focus there, and the address can be
/// shared. Nothing marks the section being read; that would need a script.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Toc;
///
/// let contents = Toc::new("On this page")
///     .item("Delivery", "delivery")
///     .sub("Abroad", "delivery-abroad")
///     .item("Returns", "returns");
/// let section = Heading::h2("Delivery").id("delivery");
/// ```
pub struct Toc {
    label: Text,
    entries: Vec<Entry>,
}

impl Toc {
    /// A table of contents with an accessible name, such as "On this page".
    pub fn new(label: impl Into<Text>) -> Self {
        Toc {
            label: label.into(),
            entries: Vec::new(),
        }
    }

    fn push(mut self, title: Text, id: &str, sub: bool) -> Self {
        let id = anchor(id);
        if !id.is_empty() && self.entries.len() < MAX_TOC_ITEMS {
            self.entries
                .push(Entry { title, id, sub });
        }
        self
    }

    /// A link to the heading with the anchor `id`.
    pub fn item(self, title: impl Into<Text>, id: &str) -> Self {
        self.push(title.into(), id, false)
    }

    /// A link one level under the entry before it. Without an entry before it, it is a plain
    /// entry.
    pub fn sub(self, title: impl Into<Text>, id: &str) -> Self {
        let sub = !self.entries.is_empty();
        self.push(title.into(), id, sub)
    }
}

impl Component for Toc {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<nav class="z-toc" aria-label="{}"><ol class="z-toc-list">"#,
            r.text(&self.label)
        ));
        let mut open_item = false;
        let mut in_sub = false;
        for entry in &self.entries {
            let link = format!(
                r##"<a class="z-toc-link" href="#{}">{}</a>"##,
                entry.id,
                r.text(&entry.title)
            );
            match (entry.sub, in_sub) {
                (true, false) => {
                    out.push_str(r#"<ol class="z-toc-list z-toc-sub"><li>"#);
                    in_sub = true;
                }
                (true, true) => out.push_str("</li><li>"),
                (false, true) => {
                    out.push_str("</li></ol></li><li>");
                    in_sub = false;
                }
                (false, false) if open_item => out.push_str("</li><li>"),
                (false, false) => out.push_str("<li>"),
            }
            out.push_str(&link);
            open_item = true;
        }
        if in_sub {
            out.push_str("</li></ol>");
        }
        if open_item {
            out.push_str("</li>");
        }
        out.push_str("</ol></nav>");
    }

    fn schema(&self) -> Json {
        let entries: Vec<Json> = self
            .entries
            .iter()
            .map(|e| json!({ "title": e.title, "id": e.id, "sub": e.sub }))
            .collect();
        json!({ "type": "toc", "label": self.label, "entries": entries })
    }
}
