//! ThemePicker examples: the theme choice inside a screen, in step with the menu's picker.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Panel::new("Appearance")
        .add(Paragraph::new(
            "The same choice as the picker in the menu: change one and the other follows.",
        ))
        .add(ThemePicker::new())
        .into()
}

/// What the catalog shows of `ThemePicker`.
pub const ENTRY: Entry = Entry {
    name: "ThemePicker",
    docs: "components/shell#theme-picker-in-a-screen",
    api: &["ThemePicker"],
    overview: &[ex!(overview)],
    examples: &[],
};
