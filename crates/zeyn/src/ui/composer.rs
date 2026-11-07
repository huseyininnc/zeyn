//! [`Composer`]: the box a user writes chat messages in, with attachments, `@` mentions searched
//! on the server, a draft kept per conversation and a send button that only works when the
//! message fits.

use super::lookup::{LookupItem, LookupQuery};
use super::thread::{MAX_CONVERSATION, MAX_MESSAGE_CHARS};
use super::{DataSource, dom_id, topic_list};
use crate::action::{Action, ActionMeta, Error, Page, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::files::{File, FileRules, valid_file_id};
use crate::i18n::Text;
use crate::model::FieldMeta;
use crate::prefs::{ClientPref, Draft};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Icon, Node};
use crate::value::FieldKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

/// Most people one mention search shows, whatever the action returns.
pub const MAX_MENTIONS: u32 = 8;

/// Longest text a mention search sends, in characters; longer text is not searched.
pub const MAX_MENTION_QUERY: usize = 50;

/// Most mentions one message may carry.
pub const MAX_MESSAGE_MENTIONS: usize = 20;

/// Longest message a [`Composer`] takes unless [`Composer::max_chars`] says otherwise.
pub const DEFAULT_MESSAGE_CHARS: usize = 4000;

/// Which key sends the message. The other one, with Enter, starts a new line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendKey {
    /// Enter sends, Shift+Enter starts a new line. The usual choice for short messages.
    #[default]
    Enter,
    /// Ctrl+Enter (Cmd+Enter on a Mac) sends, Enter starts a new line. For longer writing.
    CtrlEnter,
}

impl SendKey {
    fn code(self) -> &'static str {
        match self {
            SendKey::Enter => "enter",
            SendKey::CtrlEnter => "ctrl-enter",
        }
    }
}

/// What the send action of a [`Composer`] gets. `conversation` comes from the component, never
/// from the browser. The text is trimmed, 1 to `max_chars` characters (or empty when files are
/// attached); the files passed the upload checks of their store, at most its `max_files`, and
/// `mentions` are ids the user picked from the mention search, at most
/// [`MAX_MESSAGE_MENTIONS`]. All of it is untrusted: the action checks the user takes part in the
/// conversation and may name the people mentioned.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSend {
    pub conversation: String,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<File>,
    #[serde(default)]
    pub mentions: Vec<i64>,
}

/// What the browser posts to send.
#[derive(Deserialize)]
struct SendInput {
    #[serde(default)]
    text: String,
    #[serde(default)]
    attachments: Vec<File>,
    #[serde(default)]
    mentions: Vec<i64>,
}

/// The send action, with the limits of its composer.
struct SendSource<A> {
    conversation: String,
    max_chars: usize,
    files: Option<FileRules>,
    action: PhantomData<fn() -> A>,
}

impl<A, T> DataSource for SendSource<A>
where
    A: Action<Input = ChatSend, Output = Result<T>>,
    T: Send + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let checked = self.check(input);
        Box::pin(async move {
            let send = checked?;
            A::call(ctx, send).await??;
            Ok(json!({ "ok": true }))
        })
    }
}

impl<A> SendSource<A> {
    /// Reads the browser's input and refuses what does not fit the composer.
    fn check(&self, input: Json) -> Result<ChatSend> {
        let input: SendInput = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
        let text = input.text.trim();
        let files = match self.files {
            Some(_) => input.attachments,
            None if input.attachments.is_empty() => Vec::new(),
            None => return Err(Error::NotFound),
        };
        let max_files = self
            .files
            .map_or(0, |r| r.files() as usize);
        let fits = (!text.is_empty() || !files.is_empty())
            && text.chars().count() <= self.max_chars
            && files.len() <= max_files
            && files.iter().all(|f| {
                valid_file_id(&f.id) && f.name.chars().count() <= crate::files::MAX_FILE_NAME
            })
            && input.mentions.len() <= MAX_MESSAGE_MENTIONS;
        if !fits {
            return Err(Error::NotFound);
        }
        let mut mentions = input.mentions;
        mentions.sort_unstable();
        mentions.dedup();
        Ok(ChatSend {
            conversation: self.conversation.clone(),
            text: text.to_string(),
            attachments: files,
            mentions,
        })
    }
}

/// The mention search: at most [`MAX_MENTIONS`] people for at most [`MAX_MENTION_QUERY`]
/// characters of text.
struct MentionSource<A>(PhantomData<fn() -> A>);

impl<A> DataSource for MentionSource<A>
where
    A: Action<Input = LookupQuery, Output = Result<Page<LookupItem>>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let text = input
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if text.chars().count() > MAX_MENTION_QUERY {
                return Err(Error::NotFound);
            }
            let query = LookupQuery {
                text,
                limit: MAX_MENTIONS,
                ..LookupQuery::default()
            };
            let page = A::call(ctx, query).await??;
            let items: Vec<Json> = page
                .items
                .into_iter()
                .take(MAX_MENTIONS as usize)
                .map(|i| json!({ "id": i.id, "label": i.label, "hint": i.hint }))
                .collect();
            Ok(json!({ "items": items }))
        })
    }
}

/// The attachments control is an upload field of the store's rules.
const ATTACH: FieldMeta = FieldMeta {
    model: "zeyn.ui.composer",
    name: "attachments",
    label: "Attachments",
    kind: FieldKind::Custom { editor: "files" },
    rules: &[],
    key: false,
    readonly: false,
    version: false,
    optional: true,
    mask: None,
};

/// The box a user writes chat messages in.
///
/// A text area that grows with its text up to a maximum height, then scrolls. [`SendKey::Enter`]
/// (the default) sends with Enter and starts a new line with Shift+Enter;
/// [`SendKey::CtrlEnter`] sends with Ctrl+Enter. The send button is disabled while the message
/// is empty or longer than [`Composer::max_chars`], with a counter near the limit. Sending goes
/// through `POST /_zeyn/data` (CSRF, the write limit, the screen's access and the send action's
/// permission) with a [`ChatSend`]; a composer the user may not send with is left out of the page.
/// After a send the composer names its topics, so a [`MessageThread`] of the same conversation
/// shows the message (and streams its AI reply when it has one).
///
/// [`Composer::attachments`] adds an upload field of a store registered with `Feature::files`:
/// files upload one by one with progress, and their ids go with the message.
/// [`Composer::mentions`] searches people after `@` with a lookup action, at most
/// [`MAX_MENTIONS`] results, 200 ms after the last key, a newer search cancelling the older one.
/// [`Composer::remember`] keeps the unsent text per conversation in the client preference store.
///
/// [`MessageThread`]: crate::ui::MessageThread
///
/// ```ignore
/// Composer::of(send_message)
///     .conversation(params.id.to_string())
///     .attachments(CHAT_FILES)
///     .mentions(search_people)
///     .remember(CHAT_DRAFTS)
/// ```
///
/// Which key sends is a [`SendKey`], never a flag:
///
/// ```compile_fail
/// # use zeyn::prelude::*;
/// # use zeyn::ui::{ChatSend, Composer};
/// # #[action(public)]
/// # pub async fn send(_m: ChatSend) -> Result<()> { Ok(()) }
/// let c = Composer::of(send).send_key(true);
/// ```
///
/// Drafts take a draft key; a choice key does not compile:
///
/// ```compile_fail
/// # use std::time::Duration;
/// # use zeyn::prelude::*;
/// # use zeyn::ui::{ChatSend, Composer};
/// # use zeyn::{ClientPref, Choice, Purpose};
/// # #[action(public)]
/// # pub async fn send(_m: ChatSend) -> Result<()> { Ok(()) }
/// const TAB: ClientPref<Choice> = ClientPref::choice("tab", Duration::from_secs(60), Purpose::Preference);
/// let c = Composer::of(send).remember(TAB);
/// ```
pub struct Composer {
    send: Box<dyn DataSource>,
    send_meta: ActionMeta,
    conversation: String,
    max_chars: usize,
    send_key: SendKey,
    files: Option<FileRules>,
    mentions: Option<Box<dyn DataSource>>,
    draft: Option<ClientPref<Draft>>,
    placeholder: Option<Text>,
    topics: Vec<&'static str>,
    builder: fn(&Composer) -> Box<dyn DataSource>,
}

impl Composer {
    pub fn of<A, T>(_send: A) -> Self
    where
        A: Action<Input = ChatSend, Output = Result<T>>,
        T: Send + 'static,
    {
        fn build<A, T>(c: &Composer) -> Box<dyn DataSource>
        where
            A: Action<Input = ChatSend, Output = Result<T>>,
            T: Send + 'static,
        {
            Box::new(SendSource::<A> {
                conversation: c.conversation.clone(),
                max_chars: c.max_chars,
                files: c.files,
                action: PhantomData,
            })
        }
        let mut c = Composer {
            send: Box::new(SendSource::<A> {
                conversation: String::new(),
                max_chars: DEFAULT_MESSAGE_CHARS,
                files: None,
                action: PhantomData,
            }),
            send_meta: A::META,
            conversation: String::new(),
            max_chars: DEFAULT_MESSAGE_CHARS,
            send_key: SendKey::Enter,
            files: None,
            mentions: None,
            draft: None,
            placeholder: None,
            topics: vec!["chat"],
            builder: build::<A, T>,
        };
        c.rebuild();
        c
    }

    /// Builds the send source again after a limit changed.
    fn rebuild(&mut self) {
        self.send = (self.builder)(self);
    }

    /// The conversation messages go to, passed to the send action as
    /// [`ChatSend::conversation`]. At most [`MAX_CONVERSATION`] characters.
    pub fn conversation(mut self, id: impl Into<String>) -> Self {
        let id: String = id.into();
        self.conversation = id
            .chars()
            .take(MAX_CONVERSATION)
            .collect();
        self.rebuild();
        self
    }

    /// Longest message, 1 to `MAX_MESSAGE_CHARS` characters; 4,000 unless set.
    pub fn max_chars(mut self, n: usize) -> Self {
        self.max_chars = n.clamp(1, MAX_MESSAGE_CHARS);
        self.rebuild();
        self
    }

    /// Which key sends the message; [`SendKey::Enter`] unless set.
    pub fn send_key(mut self, key: SendKey) -> Self {
        self.send_key = key;
        self
    }

    /// Files can be attached, uploaded to the store of `rules` (registered with
    /// `Feature::files`, whose write permission the upload needs).
    pub fn attachments(mut self, rules: FileRules) -> Self {
        self.files = Some(rules);
        self.rebuild();
        self
    }

    /// `@` opens a list of people found by `search`, a lookup action (`LookupQuery` in, a page
    /// of `LookupItem` out) that gets at most [`MAX_MENTIONS`] as its limit.
    pub fn mentions<A>(mut self, _search: A) -> Self
    where
        A: Action<Input = LookupQuery, Output = Result<Page<LookupItem>>>,
    {
        self.mentions = Some(Box::new(MentionSource::<A>(PhantomData)));
        self
    }

    /// Keeps the unsent text of each conversation under a draft key registered with
    /// `App::client_prefs`, until it is sent.
    pub fn remember(mut self, key: ClientPref<Draft>) -> Self {
        self.draft = Some(key);
        self
    }

    /// The hint in the empty text area; "Write a message" unless set.
    pub fn placeholder(mut self, text: impl Into<Text>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// The topics named after a message is sent, `"chat"` unless set.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        self.topics = topic_list(topics);
        self
    }

    fn texts(&self, r: &Render) -> Json {
        let t = |k: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.composer.{k}")))
        };
        json!({
            "left": t("left"),
            "over": t("over"),
            "sent": t("sent"),
            "failed": t("failed"),
            "people": t("people"),
            "none": t("no_people"),
            "restored": t("restored"),
        })
    }
}

impl Component for Composer {
    fn render(&self, out: &mut String, r: &Render) {
        if !r.can(&self.send_meta.access) {
            return;
        }
        let id = dom_id(r, "composer");
        let t = |k: &str| r.text(&Text::key(format!("zeyn.ui.composer.{k}")));
        let placeholder = self
            .placeholder
            .as_ref()
            .map_or_else(|| t("placeholder"), |p| r.text(p));
        let mentions = self
            .mentions
            .as_ref()
            .filter(|m| r.can(&m.action().access))
            .is_some();
        let draft = self
            .draft
            .map(|k| k.attrs(r))
            .unwrap_or_default();
        let hint = match self.send_key {
            SendKey::Enter => t("hint_enter"),
            SendKey::CtrlEnter => t("hint_ctrl"),
        };
        out.push_str(&format!(
            r#"<form class="z-composer" data-zeyn-composer data-room="{room}" data-topics="{topics}" data-send-key="{key}" data-max="{max}" data-texts="{texts}"{mention}{draft} novalidate aria-label="{label}" data-testid="{scope}.composer">"#,
            room = escape(&self.conversation),
            topics = escape(&self.topics.join(",")),
            key = self.send_key.code(),
            max = self.max_chars,
            texts = attr(&self.texts(r)),
            mention = if mentions { " data-mentions" } else { "" },
            label = t("label"),
            scope = r.scope,
        ));
        if let Some(rules) = &self.files {
            out.push_str(r#"<div class="z-composer-files">"#);
            crate::files::render_upload(
                out,
                r,
                &ATTACH,
                &format!("{id}-files"),
                rules,
                true,
                &Json::Null,
            );
            out.push_str("</div>");
        }
        out.push_str(&format!(
            r#"<div class="z-composer-row"><label class="z-sr-only" for="{id}-text">{label}</label><textarea class="z-composer-text" id="{id}-text" name="text" rows="1" dir="auto" placeholder="{placeholder}" aria-describedby="{id}-hint {id}-count" data-testid="{scope}.composer.text"{auto}></textarea>"#,
            label = t("label"),
            scope = r.scope,
            auto = if mentions {
                format!(
                    r#" aria-autocomplete="list" aria-controls="{id}-people""#
                )
            } else {
                String::new()
            },
        ));
        out.push_str(&format!(
            r#"<button type="submit" class="z-button z-primary z-composer-send" aria-disabled="true" data-testid="{scope}.composer.send">{icon}<span>{send}</span></button></div>"#,
            scope = r.scope,
            icon = Icon::ArrowUp.html(),
            send = t("send"),
        ));
        if mentions {
            out.push_str(&format!(
                r#"<ul class="z-composer-people" id="{id}-people" role="listbox" aria-label="{}" hidden></ul>"#,
                t("people_label")
            ));
        }
        out.push_str(&format!(
            r#"<p class="z-composer-foot"><span class="z-composer-hint" id="{id}-hint">{hint}</span><span class="z-composer-count" id="{id}-count" aria-live="polite"></span></p><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-composer-status></p></form>"#
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "composer",
            "action": self.send_meta,
            "conversation": self.conversation,
            "max_chars": self.max_chars,
            "send_key": self.send_key,
            "attachments": self.files.map(|f| f.store()),
            "mentions": self.mentions.as_ref().map(|m| m.action()),
            "draft": self.draft.map(|d| d.info().name),
            "topics": self.topics,
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        match key {
            "send" => Some(self.send.as_ref()),
            "mentions" => self.mentions.as_deref(),
            _ => None,
        }
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.drives(t);
        }
    }
}

impl From<Composer> for Node {
    fn from(c: Composer) -> Self {
        Node::new(c)
    }
}
