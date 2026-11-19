//! Indicator examples: a count or a dot on the corner of another component.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::AvatarSize;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(
            Indicator::on(Avatar::new("Ada Lovelace"))
                .count(3)
                .label("3 unread messages"),
        )
        .col(
            Indicator::on(Avatar::new("Alan Turing"))
                .count(128)
                .label("More than 99 unread messages"),
        )
        .col(
            Indicator::on(Avatar::new("Grace Hopper"))
                .dot()
                .tone(Tone::Success)
                .label("Online"),
        )
        .into()
}

pub fn tones_and_sizes(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(
            Indicator::on(Avatar::new("İlknur Şahin").size(AvatarSize::Large))
                .count(7)
                .tone(Tone::Brand)
                .label("7 open tasks"),
        )
        .col(
            Indicator::on(Badge::new("Drafts"))
                .dot()
                .tone(Tone::Warning)
                .label("Has unsaved changes"),
        )
        .col(
            Indicator::on(Avatar::new("Ömer Çelik").size(AvatarSize::Small))
                .count(0)
                .label("No unread messages"),
        )
        .into()
}

/// What the catalog shows of `Indicator`.
pub const ENTRY: Entry = Entry {
    name: "Indicator",
    docs: "components/display#indicator",
    api: &["Indicator"],
    overview: &[ex!(overview)],
    examples: &[ex!(tones_and_sizes)],
};
