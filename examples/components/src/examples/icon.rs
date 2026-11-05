//! Icon examples: every icon of the set, shown in a toolbox with its name.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    [
        ("plus", "Plus", Icon::Plus),
        ("edit", "Edit", Icon::Edit),
        ("delete", "Delete", Icon::Delete),
        ("save", "Save", Icon::Save),
        ("search", "Search", Icon::Search),
        ("box", "Box", Icon::Box),
        ("cart", "Cart", Icon::Cart),
        ("table", "Table", Icon::Table),
        ("home", "Home", Icon::Home),
        ("chart", "Chart", Icon::Chart),
    ]
    .into_iter()
    .fold(Toolbox::new("Icons"), |bar, (key, name, icon)| {
        bar.tool(key, name, Some(icon))
    })
    .into()
}

pub fn admin(_ctx: &Ctx) -> Node {
    [
        ("users", "Users", Icon::Users),
        ("settings", "Settings", Icon::Settings),
        ("bell", "Bell", Icon::Bell),
        ("shield", "Shield", Icon::Shield),
        ("clock", "Clock", Icon::Clock),
        ("tag", "Tag", Icon::Tag),
        ("mail", "Mail", Icon::Mail),
        ("sign_out", "SignOut", Icon::SignOut),
        ("arrow_up", "ArrowUp", Icon::ArrowUp),
        ("arrow_down", "ArrowDown", Icon::ArrowDown),
    ]
    .into_iter()
    .fold(Toolbox::new("Admin icons"), |bar, (key, name, icon)| {
        bar.tool(key, name, Some(icon))
    })
    .into()
}

/// What the catalog shows of `Icon`.
pub const ENTRY: Entry = Entry {
    name: "Icon",
    docs: "language/screens#toolbar-and-context-menu",
    api: &["Icon"],
    overview: &[ex!(overview), ex!(admin)],
    examples: &[],
};
