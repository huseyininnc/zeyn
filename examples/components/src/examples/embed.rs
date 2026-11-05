//! Embed examples: a page of another site in a locked frame.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Embed::url("https://www.openstreetmap.org/export/embed.html?bbox=28.95,41.0,29.05,41.05")
        .title("Map of the old town")
        .aspect(16, 9)
        .click_to_load("")
        .notice("The map is served by OpenStreetMap.")
        .into()
}

/// The confirmed map notice, kept for a day.
pub const MAP_NOTICE: zeyn::ClientPref<zeyn::Consent> = zeyn::ClientPref::consent(
    "catalog-map",
    std::time::Duration::from_secs(24 * 60 * 60),
    zeyn::Purpose::Necessary,
);

pub fn remembered(_ctx: &Ctx) -> Node {
    Embed::url("https://www.openstreetmap.org/export/embed.html?bbox=28.97,41.0,29.0,41.02")
        .title("Map of the harbor")
        .notice("Asked once a day: the map is served by OpenStreetMap.")
        .remember(MAP_NOTICE)
        .into()
}

/// What the catalog shows of `Embed`.
pub const ENTRY: Entry = Entry {
    name: "Embed",
    docs: "components/display#embed",
    api: &["Embed"],
    overview: &[ex!(overview)],
    examples: &[ex!(remembered)],
};
