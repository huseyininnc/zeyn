//! Divider examples: a line between parts, plain, with a label or between columns.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(Paragraph::new("Sign in with your work account."))
        .add(Divider::new().label("or"))
        .add(Paragraph::new("Use a one time code sent to your email."))
        .into()
}

pub fn plain(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(Paragraph::new("Billing address"))
        .add(Divider::new())
        .add(Paragraph::new("Shipping address"))
        .into()
}

pub fn between_columns(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(Badge::new("12 open"))
        .col(Divider::new().vertical())
        .col(Badge::new("3 late").tone(Tone::Warning))
        .col(Divider::new().vertical())
        .col(Badge::new("48 closed").tone(Tone::Success))
        .into()
}

/// What the catalog shows of `Divider`.
pub const ENTRY: Entry = Entry {
    name: "Divider",
    docs: "components/content#divider",
    api: &["Divider"],
    overview: &[ex!(overview)],
    examples: &[ex!(plain), ex!(between_columns)],
};
