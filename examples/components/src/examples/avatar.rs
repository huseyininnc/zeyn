//! Avatar examples: pictures or initials of people.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::AvatarSize;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Avatar::new("Ada Lovelace").image(crate::data::picture(3)))
        .col(Avatar::new("Alan Turing"))
        .col(Avatar::new("İlknur Şahin"))
        .into()
}

pub fn sizes(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Avatar::new("Grace Hopper").size(AvatarSize::Small))
        .col(Avatar::new("Grace Hopper").size(AvatarSize::Medium))
        .col(Avatar::new("Grace Hopper").size(AvatarSize::Large))
        .into()
}

/// What the catalog shows of `Avatar`.
pub const ENTRY: Entry = Entry {
    name: "Avatar",
    docs: "components/content#avatar",
    api: &["Avatar"],
    overview: &[ex!(overview)],
    examples: &[ex!(sizes)],
};
