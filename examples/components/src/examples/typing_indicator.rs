//! TypingIndicator examples: who else is typing, under a composer. The catalog runs without the
//! realtime channel, so the indicator renders nothing here and the composer sends no signals;
//! an app with `App::socket` and the topic declared shows the names.

use crate::examples::composer::{find_people, send_sample};
use crate::examples::message_thread::thread;
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{Composer, TypingIndicator};

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(thread("support").title("Support"))
        .add(
            TypingIndicator::new("chat")
                .conversation("support")
                .person("ada", "Ada Lovelace")
                .person("grace", "Grace Hopper"),
        )
        .add(
            Composer::of(send_sample)
                .conversation("support")
                .mentions(find_people),
        )
        .add(Alert::info(t!("cx.typing.realtime")))
        .into()
}

/// What the catalog shows of `TypingIndicator`.
pub const ENTRY: Entry = Entry {
    name: "TypingIndicator",
    docs: "components/chat#typingindicator",
    api: &["TypingIndicator"],
    overview: &[ex!(overview)],
    examples: &[],
};
