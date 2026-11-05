//! LinkButton examples: a real link to another page that looks like a button.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(LinkButton::new("Card", crate::card, ()).primary())
        .col(LinkButton::new("Breadcrumb", crate::breadcrumb, ()))
        .into()
}

/// What the catalog shows of `LinkButton`.
pub const ENTRY: Entry = Entry {
    name: "LinkButton",
    docs: "components/display#linkbutton",
    api: &["LinkButton"],
    overview: &[ex!(overview)],
    examples: &[],
};
