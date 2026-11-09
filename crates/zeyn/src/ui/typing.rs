//! [`TypingIndicator`]: "Ada is typing" over the realtime channel (feature `realtime`).

use super::thread::MAX_CONVERSATION;
use crate::render::Render;
#[cfg(feature = "realtime")]
use crate::render::{attr, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

/// Most people a [`TypingIndicator`] knows by name; others show as "someone".
pub const MAX_TYPING_PEOPLE: usize = 200;

/// Shortest time between two typing signals of one user, in seconds. Later ones are dropped by
/// the browser and again by the server.
pub const TYPING_EVERY_SECONDS: u64 = 3;

/// How long a typing signal is shown without a newer one, in seconds.
pub const TYPING_SHOWN_SECONDS: u64 = 6;

/// Who is typing in a conversation, under a [`Composer`] or a [`MessageThread`]: "Ada is
/// typing", "Ada and Grace are typing", "3 people are typing", in a polite live region.
///
/// It works over the realtime channel (feature `realtime`, `App::realtime`) on a topic declared
/// with `Realtime::topic`; every subscriber of the topic needs its permission. While the user
/// types in a [`Composer`] of the same conversation, the browser sends one small signal on the
/// page's live socket at most every [`TYPING_EVERY_SECONDS`]; the server drops faster ones per
/// user and passes the signal to the other subscribers of the topic in the tenant, never back
/// to the same user. A signal carries the conversation id and a key of the user that only this
/// server can make, never the user id or any text. A name shows for [`TYPING_SHOWN_SECONDS`]
/// after the last signal. Without the `realtime` feature, or when the app has no realtime
/// channel, the indicator renders nothing and nothing is sent.
///
/// Signals reach every subscriber of the topic in the tenant: the conversation id is visible to
/// them, so use opaque ids, or a topic per audience.
///
/// [`Composer`]: crate::ui::Composer
/// [`MessageThread`]: crate::ui::MessageThread
///
/// ```ignore
/// TypingIndicator::new("chat")
///     .conversation(params.id.to_string())
///     .person("ada", "Ada Lovelace")
///     .person("grace", "Grace Hopper")
/// ```
pub struct TypingIndicator {
    topic: &'static str,
    conversation: String,
    people: Vec<(String, String)>,
}

impl TypingIndicator {
    /// An indicator on `topic`, a topic declared with `Realtime::topic`.
    pub fn new(topic: &'static str) -> Self {
        TypingIndicator {
            topic,
            conversation: String::new(),
            people: Vec::new(),
        }
    }

    /// The conversation, the same as its [`Composer`](crate::ui::Composer)'s.
    pub fn conversation(mut self, id: impl Into<String>) -> Self {
        let id: String = id.into();
        self.conversation = id
            .chars()
            .take(MAX_CONVERSATION)
            .collect();
        self
    }

    /// A person of the conversation: the user id and the name shown. At most
    /// [`MAX_TYPING_PEOPLE`]; others show as "someone".
    pub fn person(mut self, user: impl Into<String>, name: impl Into<String>) -> Self {
        if self.people.len() < MAX_TYPING_PEOPLE {
            self.people
                .push((user.into(), name.into()));
        }
        self
    }

    #[cfg(feature = "realtime")]
    fn texts(r: &Render) -> Json {
        let t = |k: &str| {
            r.i18n.text(
                r.locale,
                &crate::i18n::Text::key(format!("zeyn.ui.typing.{k}")),
            )
        };
        json!({
            "one": t("one"),
            "two": t("two"),
            "many": t("many"),
            "someone": t("someone"),
        })
    }
}

impl Component for TypingIndicator {
    #[cfg(feature = "realtime")]
    fn render(&self, out: &mut String, r: &Render) {
        let Some(ctx) = r.ctx else {
            return;
        };
        let Some(hub) = ctx.service::<crate::realtime::Hub>() else {
            return;
        };
        let Some(user) = ctx.identity().user.as_deref() else {
            return;
        };
        let tenant = ctx
            .tenant()
            .map(|t| t.id().to_string())
            .unwrap_or_default();
        let people: serde_json::Map<String, Json> = self
            .people
            .iter()
            .map(|(u, name)| (hub.person_key(&tenant, u), json!(name)))
            .collect();
        out.push_str(&format!(
            r#"<p class="z-typing" data-zeyn-typing data-zeyn-live="{topic}" data-zeyn-live-self data-room="{room}" data-self="{me}" data-people="{people}" data-texts="{texts}" aria-live="polite" data-testid="{scope}.typing"></p>"#,
            topic = escape(self.topic),
            room = escape(&self.conversation),
            me = hub.person_key(&tenant, user),
            people = attr(&people),
            texts = attr(&Self::texts(r)),
            scope = r.scope,
        ));
    }

    #[cfg(not(feature = "realtime"))]
    fn render(&self, _out: &mut String, _r: &Render) {}

    fn schema(&self) -> Json {
        json!({
            "type": "typing_indicator",
            "topic": self.topic,
            "conversation": self.conversation,
            "people": self.people.len(),
        })
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        check.listens(self.topic);
    }
}

impl From<TypingIndicator> for Node {
    fn from(t: TypingIndicator) -> Self {
        Node::new(t)
    }
}
