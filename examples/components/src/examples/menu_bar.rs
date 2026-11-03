//! MenuBar examples: a menu bar of commands (W3C APG menubar).

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    let hello = Command::effect(
        "cx.hello",
        "Say hello",
        Effect::Toast {
            message: "Hello".into(),
        },
    );
    let home = Command::navigate(
        "cx.home",
        "All components",
        crate::catalog_home,
        Default::default(),
    );
    MenuBar::new("Main menu")
        .menu("File", |m| m.command(hello))
        .menu("Go", |m| m.command(home))
        .into()
}

/// What the catalog shows of `MenuBar`.
pub const ENTRY: Entry = Entry {
    name: "MenuBar",
    docs: "components/shell",
    api: &["MenuBar", "MenuList"],
    overview: &[ex!(overview)],
    examples: &[],
};
