//! RelativeTime examples: moments shown as "5 minutes ago".

use crate::{Entry, ex};
use zeyn::prelude::*;

const HOUR_MS: i64 = 60 * 60 * 1000;

pub fn overview(ctx: &Ctx) -> Node {
    RelativeTime::new(ctx.now().add_ms(-HOUR_MS / 12)).into()
}

pub fn past_and_future(ctx: &Ctx) -> Node {
    Stack::new()
        .gap(2)
        .add(RelativeTime::new(ctx.now().add_ms(-72 * HOUR_MS)))
        .add(RelativeTime::new(ctx.now().add_ms(2 * HOUR_MS)))
        .into()
}

/// What the catalog shows of `RelativeTime`.
pub const ENTRY: Entry = Entry {
    name: "RelativeTime",
    docs: "components/content#relativetime",
    api: &["RelativeTime"],
    overview: &[ex!(overview)],
    examples: &[ex!(past_and_future)],
};
