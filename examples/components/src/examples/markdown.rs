//! Markdown examples: safe Markdown rendered on the server.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Markdown::new("**Bold**, *italic* and `code`.\n\n- One\n- Two\n\n[A link](https://example.com)").into()
}

/// What the catalog shows of `Markdown`.
pub const ENTRY: Entry = Entry {
    name: "Markdown",
    docs: "components/display#markdown",
    api: &["Markdown"],
    overview: &[ex!(overview)],
    examples: &[],
};
