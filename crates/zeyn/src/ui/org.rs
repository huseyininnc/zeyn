//! [`OrgChart`]: a hierarchy as boxes joined by lines.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(OrgChart);

/// Most boxes an [`OrgChart`] draws; the rest are left out.
pub const MAX_ORG_NODES: usize = 200;

/// Most levels an [`OrgChart`] draws; deeper boxes are left out.
pub const MAX_ORG_DEPTH: usize = 8;

/// A box of an [`OrgChart`]: a name, an optional line under it and the boxes below it.
pub struct OrgNode {
    name: Text,
    role: Option<Text>,
    children: Vec<OrgNode>,
}

impl OrgNode {
    pub fn new(name: impl Into<Text>) -> Self {
        OrgNode {
            name: name.into(),
            role: None,
            children: Vec::new(),
        }
    }

    /// A quieter line under the name, such as a job title.
    pub fn role(mut self, role: impl Into<Text>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Adds a box below this one.
    pub fn child(mut self, child: OrgNode) -> Self {
        self.children.push(child);
        self
    }

    fn schema(&self) -> Json {
        json!({
            "name": self.name,
            "role": self.role,
            "children": self.children.iter().map(OrgNode::schema).collect::<Vec<_>>(),
        })
    }
}

/// A hierarchy as boxes joined by lines, from one box at the top: the people of a company, the
/// parts of a product. It is nested lists, so screen readers get the levels, and the lines are
/// drawn by the stylesheet. A wide chart scrolls inside its own box, which takes focus.
///
/// ```
/// use zeyn::ui::{OrgChart, OrgNode};
/// let company = OrgChart::new(
///     "Company",
///     OrgNode::new("Ada Lovelace")
///         .role("Director")
///         .child(OrgNode::new("Alan Turing").role("Engineering"))
///         .child(OrgNode::new("Grace Hopper").role("Operations")),
/// );
/// ```
pub struct OrgChart {
    label: Text,
    root: OrgNode,
}

impl OrgChart {
    /// A chart named by `label` under the box `root`.
    pub fn new(label: impl Into<Text>, root: OrgNode) -> Self {
        OrgChart {
            label: label.into(),
            root,
        }
    }
}

fn write(out: &mut String, r: &Render, node: &OrgNode, depth: usize, left: &mut usize) {
    if *left == 0 {
        return;
    }
    *left -= 1;
    let role = node
        .role
        .as_ref()
        .map(|t| format!(r#"<span class="z-org-role">{}</span>"#, r.text(t)))
        .unwrap_or_default();
    out.push_str(&format!(
        r#"<li><div class="z-org-box"><span class="z-org-name">{}</span>{role}</div>"#,
        r.text(&node.name)
    ));
    if !node.children.is_empty() && depth + 1 < MAX_ORG_DEPTH && *left > 0 {
        out.push_str("<ul>");
        for child in &node.children {
            write(out, r, child, depth + 1, left);
        }
        out.push_str("</ul>");
    }
    out.push_str("</li>");
}

impl Component for OrgChart {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-org" role="group" aria-label="{}" tabindex="0"><ul>"#,
            r.text(&self.label)
        ));
        let mut left = MAX_ORG_NODES;
        write(out, r, &self.root, 0, &mut left);
        out.push_str("</ul></div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "org_chart", "label": self.label, "root": self.root.schema() })
    }
}
