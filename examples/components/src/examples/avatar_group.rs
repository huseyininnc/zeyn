//! AvatarGroup examples: the people of one thing, with a count for the rest.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::AvatarSize;

/// A group of the sample people.
fn team(label: &'static str) -> AvatarGroup {
    [
        "Ada Lovelace",
        "Alan Turing",
        "İlknur Şahin",
        "Grace Hopper",
        "Ömer Çelik",
        "Edsger Dijkstra",
        "Barbara Liskov",
    ]
    .into_iter()
    .fold(AvatarGroup::new(label), |group, name| {
        group.add(Avatar::new(name))
    })
}

pub fn overview(_ctx: &Ctx) -> Node {
    team("Assignees").into()
}

pub fn small_and_short(_ctx: &Ctx) -> Node {
    team("Reviewers")
        .max(3)
        .size(AvatarSize::Small)
        .into()
}

pub fn with_pictures(_ctx: &Ctx) -> Node {
    AvatarGroup::new("Authors")
        .add(Avatar::new("Ada Lovelace").image(crate::data::picture(3)))
        .add(Avatar::new("Alan Turing").image(crate::data::picture(5)))
        .add(Avatar::new("Grace Hopper"))
        .size(AvatarSize::Large)
        .into()
}

/// What the catalog shows of `AvatarGroup`.
pub const ENTRY: Entry = Entry {
    name: "AvatarGroup",
    docs: "components/content#avatargroup",
    api: &["AvatarGroup"],
    overview: &[ex!(overview)],
    examples: &[ex!(small_and_short), ex!(with_pictures)],
};
