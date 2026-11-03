//! Live examples of the components that react to the reader: ratings, relative times, share
//! buttons, chips, heroes, reactions and card previews.

use zeyn::prelude::*;

/// A rating of the docs example.
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

/// Accepts a rating and sends the stars back. Nothing is stored.
#[action(public)]
pub async fn send_feedback(feedback: Valid<Feedback>) -> Result<i64> {
    Ok(feedback.stars.unwrap_or(0))
}

/// A form with one rating field.
pub fn rating() -> Node {
    Form::of(Feedback { stars: Some(4) })
        .field(Feedback::stars)
        .control(Feedback::stars, Control::Rating)
        .submit(t!("docs.live.send"), send_feedback)
        .on_success(|_, ui| ui.toast(t!("docs.live.saved")))
        .into()
}

/// A relative time five minutes back and a share button.
pub fn engage(ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(RelativeTime::new(ctx.now().add_ms(-5 * 60 * 1000)))
        .col(Share::page())
        .into()
}
