//! Side menu examples: the menu of the app shell, with badges and counts.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    let _menu = Feature::new("mail").menu(|m| {
        m.group("Mail", Icon::Mail)
            .item("Inbox", crate::catalog_home)
            .badge("New")
    });
    Alert::info(
        "The menu on the side of this catalog is the side menu: type in its filter box, pin a \
         link to Favorites, fold it to an icon rail with the button in its bar, or make the \
         window narrow to see it as a drawer.",
    )
    .into()
}

/// What the catalog shows of the side menu.
pub const ENTRY: Entry = Entry {
    name: "SideMenu",
    docs: "components/shell",
    api: &["Menu", "MenuNode"],
    overview: &[ex!(overview)],
    examples: &[],
};
