//! Rating examples: a whole number field shown as stars.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// A rating with five stars and one with three.
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
    #[rule(min = 1, max = 3)]
    pub effort: Option<i64>,
}

/// Accepts the rating. Nothing is stored.
#[action(public)]
pub async fn send_feedback(feedback: Valid<Feedback>) -> Result<i64> {
    Ok(feedback.stars.unwrap_or(0))
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Feedback {
        stars: Some(4),
        effort: None,
    })
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit("Send", send_feedback)
    .on_success(|_, ui| ui.toast("Thanks. Nothing was stored."))
    .into()
}

pub fn three_stars(_ctx: &Ctx) -> Node {
    Form::of(Feedback {
        stars: Some(5),
        effort: None,
    })
    .field(Feedback::effort)
    .control(Feedback::effort, Control::Rating)
    .submit("Send", send_feedback)
    .into()
}

/// A rating in half stars: a `Decimal` field with one decimal.
#[model]
pub struct Taste {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
}

/// Accepts the rating. Nothing is stored.
#[action(public)]
pub async fn send_taste(taste: Valid<Taste>) -> Result<()> {
    let _ = taste;
    Ok(())
}

pub fn half_stars(_ctx: &Ctx) -> Node {
    Form::of(Taste {
        stars: Decimal::parse("3.5"),
    })
    .field(Taste::stars)
    .control(Taste::stars, Control::Rating)
    .submit("Send", send_taste)
    .into()
}

/// What the catalog shows of `Control::Rating`.
pub const ENTRY: Entry = Entry {
    name: "Rating",
    docs: "components/controls#rating",
    api: &["Form"],
    overview: &[ex!(overview)],
    examples: &[ex!(three_stars), ex!(half_stars)],
};
