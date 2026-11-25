//! The version after 0.0.13: side menu items that open a screen as a window. Rename this module
//! when the version is released under another number.

use super::{Change, Pattern, Step};

pub const STEP: Step = Step {
    version: "0.0.14",
    changes: &[Change::Review(
        Pattern::Struct("MenuItem"),
        "`zeyn::MenuItem` has a new field `target` (`MenuTarget::Page` for a link, `MenuTarget::Window { params }` for a window): add `target: MenuTarget::Page`, or build items with `Menu::item` and `Menu::window`",
    )],
};
