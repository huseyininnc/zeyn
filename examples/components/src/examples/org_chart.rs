//! OrgChart examples: a hierarchy as boxes joined by lines.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    OrgChart::new(
        "Company",
        OrgNode::new("Ada Lovelace")
            .role("Director")
            .child(
                OrgNode::new("Alan Turing")
                    .role("Engineering")
                    .child(OrgNode::new("İlknur Şahin").role("Backend"))
                    .child(OrgNode::new("Ömer Çelik").role("Frontend")),
            )
            .child(
                OrgNode::new("Grace Hopper")
                    .role("Operations")
                    .child(OrgNode::new("Barbara Liskov").role("Support")),
            )
            .child(OrgNode::new("Edsger Dijkstra").role("Finance")),
    )
    .into()
}

/// What the catalog shows of `OrgChart`.
pub const ENTRY: Entry = Entry {
    name: "OrgChart",
    docs: "components/data#orgchart",
    api: &["OrgChart", "OrgNode"],
    overview: &[ex!(overview)],
    examples: &[],
};
