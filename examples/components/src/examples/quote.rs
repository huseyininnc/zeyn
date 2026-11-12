//! Quote examples: quoted words with who said them and where.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Quote::new(
        "The Analytical Engine has no pretensions whatever to originate anything. It can do whatever we know how to order it to perform.",
    )
    .author("Ada Lovelace")
    .source("Notes on the Analytical Engine")
    .into()
}

pub fn text_only(_ctx: &Ctx) -> Node {
    Quote::new("Delivered a day early and packed with care.").into()
}

/// What the catalog shows of `Quote`.
pub const ENTRY: Entry = Entry {
    name: "Quote",
    docs: "components/content#quote",
    api: &["Quote"],
    overview: &[ex!(overview)],
    examples: &[ex!(text_only)],
};
