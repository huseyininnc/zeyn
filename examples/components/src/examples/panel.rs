//! Panel examples: a titled section that can collapse.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Panel::new("Details").add(Paragraph::new("A panel groups related content under a title.")).into()
}

pub fn collapsible(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(Panel::new("Open").collapsible().add(Paragraph::new("Press the title to fold it.")))
        .add(Panel::new("Closed").collapsible().collapsed().add(Paragraph::new("Hidden until opened.")))
        .into()
}

/// What the catalog shows of `Panel`.
pub const ENTRY: Entry = Entry {
    name: "Panel",
    docs: "components/layouts#card-and-panel",
    api: &["Panel"],
    overview: &[ex!(overview)],
    examples: &[ex!(collapsible)],
};
