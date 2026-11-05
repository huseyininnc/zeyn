//! ClearPrefs examples: a button that removes the choices this browser keeps for the app.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::ClearPrefs;

pub fn overview(_ctx: &Ctx) -> Node {
    ClearPrefs::new("Forget my choices in this browser").into()
}

/// What the catalog shows of `ClearPrefs`.
pub const ENTRY: Entry = Entry {
    name: "ClearPrefs",
    docs: "components/display#clearprefs",
    api: &["ClearPrefs"],
    overview: &[ex!(overview)],
    examples: &[],
};
