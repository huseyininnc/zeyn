//! List examples: rows with an image, a title and meta text.

use crate::data::{Person, people, search_people};
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::ListItem;

pub fn overview(_ctx: &Ctx) -> Node {
    List::of(search_people)
        .title("Team")
        .item(|p: &Person| {
            ListItem::new(p.name.clone())
                .image(crate::data::picture(p.id.0))
                .subtitle(p.role.clone())
                .meta(p.city.clone())
                .link(crate::avatar, ())
        })
        .paging(Paging::infinite(4))
        .into()
}

pub fn fixed(_ctx: &Ctx) -> Node {
    people()
        .iter()
        .take(3)
        .fold(List::new(), |list, p| {
            list.item(ListItem::new(p.name.clone()).subtitle(p.role.clone()))
        })
        .into()
}

/// What the catalog shows of `List`.
pub const ENTRY: Entry = Entry {
    name: "List",
    docs: "components/content#list",
    api: &["List", "ListItem"],
    overview: &[ex!(overview)],
    examples: &[ex!(fixed)],
};
