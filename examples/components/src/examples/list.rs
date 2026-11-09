//! List examples: rows with an image, a title and meta text.

use crate::data::{Person, Product, people, search_people, search_products};
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

pub fn numbered_pages(ctx: &Ctx) -> Node {
    let now = ctx.now();
    List::of(crate::data::people_uncounted)
        .item(move |p: &Person| {
            ListItem::new(p.name.clone())
                .subtitle(p.role.clone())
                .meta(RelativeTime::new(now.add_ms(-p.id.0 * 3_600_000)))
        })
        .paging(Paging::pages(5))
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

pub fn builder(_ctx: &Ctx) -> Node {
    List::of(search_people)
        .title("Team, built row by row")
        .builder(|p: &Person| {
            Row::new()
                .compact()
                .col(Avatar::new(p.name.clone()))
                .col(Paragraph::new(format!("{}, {}", p.role, p.city)))
        })
        .separator(Paragraph::new("·"))
        .paging(Paging::infinite(4))
        .into()
}

pub fn virtual_rows(_ctx: &Ctx) -> Node {
    List::of(search_products)
        .title("Every product, scrolled virtually")
        .builder(|p: &Product| {
            Paragraph::new(format!("{} · {} · {} in stock", p.code, p.name, p.stock))
        })
        .row_height(40)
        .paging(Paging::virtual_scroll())
        .into()
}

pub fn repeat(_ctx: &Ctx) -> Node {
    Repeat::of(people().iter().take(4), |p| Badge::new(p.name.clone()))
        .separator(Paragraph::new("then"))
        .into()
}

/// What the catalog shows of `List`.
pub const ENTRY: Entry = Entry {
    name: "List",
    docs: "components/content#list",
    api: &["List", "ListItem", "CardGrid", "Repeat"],
    overview: &[ex!(overview)],
    examples: &[
        ex!(numbered_pages),
        ex!(fixed),
        ex!(builder),
        ex!(virtual_rows),
        ex!(repeat),
    ],
};
