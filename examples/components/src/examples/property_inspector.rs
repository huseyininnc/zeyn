//! Property inspector examples: an inspector that edits what an island selects.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    PropertyInspector::island("designer").into()
}

/// What the catalog shows of `PropertyInspector`.
pub const ENTRY: Entry = Entry {
    name: "Property inspector",
    docs: "components/shell",
    api: &["PropertyInspector", "FormSchema", "FieldSpec"],
    overview: &[ex!(overview)],
    examples: &[],
};
