//! Progress examples: a labelled bar.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(Progress::new("Import", 64, 100))
        .add(Progress::new("Storage", 92, 100).tone(Tone::Warning))
        .into()
}

/// What the catalog shows of `Progress`.
pub const ENTRY: Entry = Entry {
    name: "Progress",
    docs: "components/display#progress",
    api: &["Progress"],
    overview: &[ex!(overview)],
    examples: &[],
};
