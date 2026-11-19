//! ListDetail examples: a list and the detail of its selected row.

use crate::data::{Person, search_people};
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{ListDetail, ListItem};

pub fn overview(_ctx: &Ctx) -> Node {
    ListDetail::new(
        "Team",
        List::of(search_people)
            .item(|p: &Person| ListItem::new(p.name.clone()).subtitle(p.role.clone()))
            .paging(Paging::infinite(4)),
    )
    .detail(Some(
        Card::new()
            .title("Ada Lovelace")
            .add(Paragraph::new("Engineering lead. On phones this pane replaces the list, with a back link above it.")),
    ))
    .back(LinkButton::new("Back to the team", crate::list_detail, ()))
    .into()
}

pub fn nothing_selected(_ctx: &Ctx) -> Node {
    ListDetail::new(
        "Team",
        List::of(search_people).item(|p: &Person| ListItem::new(p.name.clone())),
    )
    .detail(None::<Node>)
    .empty(EmptyState::new("Pick a person").text("Their details show here."))
    .into()
}

/// What the catalog shows of `ListDetail`.
pub const ENTRY: Entry = Entry {
    name: "ListDetail",
    docs: "components/layouts",
    api: &["ListDetail"],
    overview: &[ex!(overview)],
    examples: &[ex!(nothing_selected)],
};
