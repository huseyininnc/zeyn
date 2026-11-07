//! Countdown examples: the time left until a moment on the server's clock, and its end state.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(ctx: &Ctx) -> Node {
    let midnight = ctx
        .now()
        .add_ms(26 * 3_600_000 + 17 * 60_000);
    Countdown::until(midnight)
        .label("Sale ends in")
        .ended(Badge::new("The sale has ended"))
        .into()
}

pub fn short(ctx: &Ctx) -> Node {
    Countdown::until(ctx.now().add_ms(75_000))
        .label("Offer ends in")
        .ended(Badge::new("The offer has ended").tone(Tone::Warning))
        .into()
}

/// What the catalog shows of `Countdown`.
pub const ENTRY: Entry = Entry {
    name: "Countdown",
    docs: "components/commerce#countdown",
    api: &["Countdown"],
    overview: &[ex!(overview)],
    examples: &[ex!(short)],
};
