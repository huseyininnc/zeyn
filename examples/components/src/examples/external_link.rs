//! ExternalLink examples: links that leave the app safely.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(ExternalLink::new("https://example.com/help", "Help center"))
        .col(ExternalLink::new("https://example.org", "Website").new_tab())
        .into()
}

/// What the catalog shows of `ExternalLink`.
pub const ENTRY: Entry = Entry {
    name: "ExternalLink",
    docs: "components/display#externallink",
    api: &["ExternalLink"],
    overview: &[ex!(overview)],
    examples: &[],
};
