//! Highlight examples: the words of a search marked in a text.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .gap(1)
        .add(Highlight::new("Green tea, loose leaf, 100 g", "tea"))
        .add(Highlight::new("Teapot with a tea strainer", "tea"))
        .add(Highlight::new("Coffee beans, dark roast", "tea"))
        .into()
}

pub fn several_words(_ctx: &Ctx) -> Node {
    Highlight::new(
        "Olive oil soap with green tea and lavender oil",
        "OIL green",
    )
    .into()
}

/// What the catalog shows of `Highlight`.
pub const ENTRY: Entry = Entry {
    name: "Highlight",
    docs: "components/content#highlight",
    api: &["Highlight"],
    overview: &[ex!(overview)],
    examples: &[ex!(several_words)],
};
