//! Breadcrumb examples: the trail to the current page.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Breadcrumb::new()
        .link("Components", crate::catalog_home, Default::default())
        .link("Navigation", crate::breadcrumb, ())
        .current("Breadcrumb")
        .into()
}

/// What the catalog shows of `Breadcrumb`.
pub const ENTRY: Entry = Entry {
    name: "Breadcrumb",
    docs: "components/display#breadcrumb",
    api: &["Breadcrumb"],
    overview: &[ex!(overview)],
    examples: &[],
};
