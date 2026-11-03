//! Timeline examples: events in order with a tone each.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(ctx: &Ctx) -> Node {
    let hour = 3_600_000;
    Timeline::new()
        .event(ctx.now().add_ms(-26 * hour), "Created", None, Tone::Info)
        .event(
            ctx.now().add_ms(-3 * hour),
            "Approved",
            Some(Text::from("by the manager")),
            Tone::Success,
        )
        .event(ctx.now(), "Shipped", None, Tone::Brand)
        .into()
}

/// What the catalog shows of `Timeline`.
pub const ENTRY: Entry = Entry {
    name: "Timeline",
    docs: "components/display#timeline",
    api: &["Timeline"],
    overview: &[ex!(overview)],
    examples: &[],
};
