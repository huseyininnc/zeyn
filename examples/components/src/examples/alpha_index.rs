//! AlphaIndex examples: a letter bar from the user's alphabet.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    AlphaIndex::new(crate::catalog_home, |letter| zeyn::ui::SearchParams { q: letter.to_string() })
        .current("C")
        .other()
        .into()
}

/// What the catalog shows of `AlphaIndex`.
pub const ENTRY: Entry = Entry {
    name: "AlphaIndex",
    docs: "components/content#alphaindex",
    api: &["AlphaIndex"],
    overview: &[ex!(overview)],
    examples: &[],
};
