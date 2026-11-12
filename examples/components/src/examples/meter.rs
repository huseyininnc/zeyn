//! Meter examples: a measurement within a range, colored by its thresholds.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{Better, Meter};

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(
            Meter::new("Disk used", 412, 0, 500)
                .high(400)
                .better(Better::Lower)
                .unit("GB"),
        )
        .add(
            Meter::new("Stock of black tea", 1_240, 0, 2_000)
                .low(300)
                .better(Better::Higher)
                .unit("kg"),
        )
        .into()
}

pub fn a_good_middle(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(
            Meter::new("Room temperature", 21, 0, 40)
                .low(18)
                .high(26)
                .unit("°C"),
        )
        .add(
            Meter::new("Cold store temperature", 31, 0, 40)
                .low(18)
                .high(26)
                .unit("°C"),
        )
        .into()
}

/// What the catalog shows of `Meter`.
pub const ENTRY: Entry = Entry {
    name: "Meter",
    docs: "components/display#meter",
    api: &["Meter"],
    overview: &[ex!(overview)],
    examples: &[ex!(a_good_middle)],
};
