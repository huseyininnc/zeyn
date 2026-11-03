//! Form examples: a form of a model with server side checks.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// A message sent from the example form.
#[model]
pub struct Message {
    #[rule(required, max_len = 60)]
    pub name: String,
    #[rule(required, email)]
    pub email: String,
    #[rule(min = 1, max = 9)]
    pub copies: i64,
    #[rule(max_len = 300)]
    pub note: Option<String>,
}

/// Accepts a valid message. Nothing is stored.
#[action(public)]
pub async fn send_message(message: Valid<Message>) -> Result<String> {
    Ok(message.name.clone())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Message { copies: 1, ..Message::default() })
        .row(|r| {
            r.field(Message::name)
                .field(Message::email)
        })
        .field(Message::copies)
        .field(Message::note)
        .submit("Send", send_message)
        .on_success(|_, ui| ui.toast("Sent. Nothing was stored."))
        .into()
}

/// What the catalog shows of `Form`.
pub const ENTRY: Entry = Entry {
    name: "Form",
    docs: "language/forms",
    api: &["Form", "FormRow"],
    overview: &[ex!(overview)],
    examples: &[],
};
