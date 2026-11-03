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

/// The tags of the chips example, joined with commas.
#[params]
pub struct TagParams {
    pub tags: String,
}

/// Filter chips over the tags of [`tagged`] and link chips to it.
pub fn chips(current: &str) -> Node {
    Stack::new()
        .gap(2)
        .add(
            Chips::filter("Tags", tagged, |tags| TagParams { tags: tags.join(",") })
                .current(current.split(','))
                .option("guide", "Guides")
                .option("news", "News")
                .option("release", "Releases")
                .multiple(),
        )
        .add(
            Chips::links("Topics")
                .link("Only guides", tagged, TagParams { tags: "guide".into() })
                .link("Only news", tagged, TagParams { tags: "news".into() }),
        )
        .into()
}

/// The screen the chips example filters: the chips again and the tags they chose.
#[screen(route = "/examples/tagged", public)]
pub fn tagged(p: TagParams) -> Screen {
    let chosen = if p.tags.is_empty() { "No tag chosen.".to_string() } else { format!("Chosen: {}", p.tags) };
    Screen::new("Chips example")
        .body(chips(&p.tags))
        .body(Paragraph::new(chosen))
}

/// A hero with three slides that rotate every eight seconds, with a pause button.
pub fn hero() -> Node {
    Hero::new("Fresh every morning")
        .text("Order before nine and it is at your door by noon.")
        .action(ActionButton::new(t!("docs.live.count"), crate::live::count_samples, ()))
        .slide("Twelve new teas")
        .text("From four countries, in small tins.")
        .slide("Free delivery")
        .text("On every order over 500.")
        .rotate(8)
        .into()
}
