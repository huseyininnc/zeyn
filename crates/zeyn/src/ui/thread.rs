//! [`MessageThread`]: the messages of one conversation as chat bubbles, with day separators,
//! grouping by sender, delivery states, attachments, a virtualized history that loads older
//! messages on scroll up, and AI replies streamed as safe Markdown ([`MessageThread::reply`]).

use super::catalog::check_masks;
use super::embed::{Fit, Image, Placeholder};
use super::{dom_id, topic_list};
use crate::action::{Action, ActionMeta, DataQuery, Error, Page, PageRequest, Result, Sort};
use crate::ctx::{BoxFuture, Ctx};
use crate::files::{File, FileRules, valid_file_id};
use crate::i18n::Text;
use crate::model::{Field, Model};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, GridSource, Icon, LoadedPage, Node};
use crate::time::{Date, DateTime};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Messages one history request loads, whatever the browser asks for.
pub const THREAD_PAGE: u32 = 50;

/// Longest message text a [`MessageThread`] shows, in characters; the rest is cut.
pub const MAX_MESSAGE_CHARS: usize = 16_000;

/// Longest conversation id, in characters.
pub const MAX_CONVERSATION: usize = 200;

/// Attachments shown under one message; more are counted in a note.
pub const MAX_SHOWN_FILES: usize = 10;

/// Longest streamed reply, in characters: the server stops forwarding after this many.
pub const MAX_REPLY_CHARS: usize = 32_000;

/// Pieces of a streamed reply held between the generator and the browser.
pub const REPLY_BUFFER: usize = 32;

/// Longest a streamed reply may run, in seconds, before the server ends it.
pub const REPLY_SECONDS: u64 = 120;

/// Messages of one sender closer together than this share one group.
const GROUP_GAP_MS: i64 = 5 * 60 * 1000;

/// Width and height of the box an image attachment is shown in, in CSS pixels.
const IMAGE_BOX: (u32, u32) = (240, 160);

/// Where a message is on its way to the other side. Own messages show it with an icon and a
/// text for screen readers; [`Delivery::Read`] is the read receipt.
#[zeyn_macros::choice]
pub enum Delivery {
    Sent,
    Sending,
    Delivered,
    Read,
    Failed,
}

impl Delivery {
    /// The locale key and class suffix: `sent`, `sending`, `delivered`, `read` or `failed`.
    pub fn code(self) -> &'static str {
        match self {
            Delivery::Sent => "sent",
            Delivery::Sending => "sending",
            Delivery::Delivered => "delivered",
            Delivery::Read => "read",
            Delivery::Failed => "failed",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Delivery::Sent => Icon::Check,
            Delivery::Delivered => Icon::CheckCircle,
            Delivery::Sending => Icon::Clock,
            Delivery::Read => Icon::Eye,
            Delivery::Failed => Icon::AlertCircle,
        }
    }
}

/// What the history action of a [`MessageThread`] gets: the conversation, set by the server from
/// the component (never by the browser), and the page wanted, newest first. The action must check
/// that `ctx.identity()` takes part in the conversation, then answer with
/// `query.apply(rows)` or the same query in SQL.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadQuery {
    pub conversation: String,
    pub query: DataQuery,
}

/// What the reply action of [`MessageThread::reply`] gets: the conversation, set by the server,
/// and the message the user just sent (1 to [`MAX_MESSAGE_CHARS`] characters, untrusted text).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyRequest {
    pub conversation: String,
    pub text: String,
}

/// A reply that streams: pieces of text from a generator, read by the server as they come and
/// sent to the browser as JSON lines. Made with [`Reply::channel`]; the action returns the
/// `Reply` and keeps the [`ReplySender`] in the task that writes.
#[cfg(feature = "server")]
pub struct Reply {
    pub(crate) key: String,
    pub(crate) pieces: tokio::sync::mpsc::Receiver<String>,
}

/// The writing end of a [`Reply`], with a bounded buffer of [`REPLY_BUFFER`] pieces.
#[cfg(feature = "server")]
pub struct ReplySender(tokio::sync::mpsc::Sender<String>);

#[cfg(feature = "server")]
impl Reply {
    /// A reply whose message will be stored with `key`, the key of the model, so the history
    /// shows the stored message in its place instead of twice.
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// use zeyn::ui::Reply;
    /// let (tx, reply) = Reply::channel("42");
    /// tokio::spawn(async move {
    ///     for word in ["Hello", " there"] {
    ///         if !tx.send(word).await {
    ///             return;
    ///         }
    ///     }
    /// });
    /// assert_eq!(reply.key(), "42");
    /// # });
    /// ```
    pub fn channel(key: impl Into<String>) -> (ReplySender, Reply) {
        let (tx, rx) = tokio::sync::mpsc::channel(REPLY_BUFFER);
        (
            ReplySender(tx),
            Reply {
                key: key.into(),
                pieces: rx,
            },
        )
    }

    /// The key the stored message will have.
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[cfg(feature = "server")]
impl ReplySender {
    /// Sends the next piece, waiting while the buffer is full. `false` once the browser is gone
    /// (the user pressed stop or left) or the reply ran into [`MAX_REPLY_CHARS`] or
    /// [`REPLY_SECONDS`]: stop generating then.
    pub async fn send(&self, piece: impl Into<String>) -> bool {
        self.0.send(piece.into()).await.is_ok()
    }
}

/// Text a component streams to the browser through an action, found by node path and key, like
/// a [`DataSource`]. The server checks CSRF, the screen's access, the action's permission and the
/// interceptors before it starts.
#[cfg(feature = "server")]
pub trait StreamSource: Send + Sync {
    fn action(&self) -> ActionMeta;

    /// Starts the action with the client's input, which is untrusted.
    fn start(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Reply>>;
}

/// The reply action of a thread, as a function of the conversation and the user's text.
#[cfg(feature = "server")]
type ReplyFn = fn(Ctx, ReplyRequest) -> BoxFuture<Result<Reply>>;

#[cfg(feature = "server")]
fn start_reply<A>(ctx: Ctx, request: ReplyRequest) -> BoxFuture<Result<Reply>>
where
    A: Action<Input = ReplyRequest, Output = Result<Reply>>,
{
    Box::pin(async move { A::call(ctx, request).await? })
}

type ThreadFetch<E> = fn(Ctx, ThreadQuery) -> BoxFuture<Result<Page<E>>>;

fn fetch_thread<A, E>(ctx: Ctx, query: ThreadQuery) -> BoxFuture<Result<Page<E>>>
where
    A: Action<Input = ThreadQuery, Output = Result<Page<E>>>,
    E: Model,
{
    Box::pin(async move { A::call(ctx, query).await? })
}

/// Hash of a sender, so the page can group messages without holding user ids.
pub(crate) fn short_hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Text cut at `max` characters.
fn clip(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

/// The messages of one conversation, oldest at the top, newest at the bottom: bubbles on the
/// end side for the user's own messages and on the start side for the others (mirrored in right
/// to left languages), consecutive messages of one sender within five minutes grouped under one
/// name, a heading for every day in the user's time zone, the time and, for own messages, the
/// [`Delivery`] state. Attachments show as images (progressive, lazy) or download links.
///
/// The rows come from an action that takes a [`ThreadQuery`] and returns a [`Page`] of the
/// model, newest first, [`THREAD_PAGE`] at a time. The first page is rendered with the screen.
/// Scrolling up loads older pages through `POST /_zeyn/grid` (the screen's access and the
/// action's permission checked each time) and keeps the message the reader looks at in place.
/// At most 200 messages stay in the page: going further drops the far end, which loads again
/// when the reader comes back. The history is an APG feed: Page Up and Page Down move between
/// messages, every message carries `aria-posinset` and `aria-setsize`.
///
/// New messages are fetched when one of the thread's topics is named (after a [`Composer`]
/// sends, after `ui.refresh("chat")`, or with [`MessageThread::live`] and the `realtime`
/// feature, `hub.notify(&ctx, "chat")`) and announced in a polite live region.
///
/// [`Composer`]: crate::ui::Composer
///
/// ```ignore
/// MessageThread::of(conversation_messages)
///     .conversation(params.id.to_string())
///     .author(Msg::author)
///     .name(Msg::author_name)
///     .text(Msg::body)
///     .when(Msg::sent_at)
///     .status(Msg::status)
///     .attachments(Msg::files, CHAT_FILES)
///     .live()
/// ```
pub struct MessageThread<E: Model> {
    action: ActionMeta,
    fetch: ThreadFetch<E>,
    conversation: String,
    author: Option<Field<E, String>>,
    name: Option<Field<E, String>>,
    text: Option<Field<E, String>>,
    when: Option<Field<E, DateTime>>,
    status: Option<Field<E, Delivery>>,
    files: Option<(Field<E, Vec<File>>, &'static str)>,
    ai: Option<Field<E, bool>>,
    title: Option<Text>,
    topics: Vec<&'static str>,
    live: bool,
    #[cfg(feature = "server")]
    reply: Option<(ActionMeta, ReplyFn)>,
}

impl<E: Model> MessageThread<E> {
    pub fn of<A>(_history: A) -> Self
    where
        A: Action<Input = ThreadQuery, Output = Result<Page<E>>>,
    {
        MessageThread {
            action: A::META,
            fetch: fetch_thread::<A, E>,
            conversation: String::new(),
            author: None,
            name: None,
            text: None,
            when: None,
            status: None,
            files: None,
            ai: None,
            title: None,
            topics: vec!["chat"],
            live: false,
            #[cfg(feature = "server")]
            reply: None,
        }
    }

    /// The conversation shown, passed to every action as [`ThreadQuery::conversation`]. Build it
    /// from the screen's params after the screen checked the user may open it. At most
    /// [`MAX_CONVERSATION`] characters.
    pub fn conversation(mut self, id: impl Into<String>) -> Self {
        let id: String = id.into();
        self.conversation = clip(&id, MAX_CONVERSATION).to_string();
        self
    }

    /// Who sent a message: the user id, compared with the signed in user to tell own messages
    /// from the others.
    pub fn author(mut self, field: Field<E, String>) -> Self {
        self.author = Some(field);
        self
    }

    /// The sender's name as shown; the author id unless set.
    pub fn name(mut self, field: Field<E, String>) -> Self {
        self.name = Some(field);
        self
    }

    /// The text of a message, shown as plain text with its line breaks.
    pub fn text(mut self, field: Field<E, String>) -> Self {
        self.text = Some(field);
        self
    }

    /// When a message was sent: the order, the day headings and the grouping.
    pub fn when(mut self, field: Field<E, DateTime>) -> Self {
        self.when = Some(field);
        self
    }

    /// The delivery state of a message, shown on the user's own messages.
    pub fn status(mut self, field: Field<E, Delivery>) -> Self {
        self.status = Some(field);
        self
    }

    /// Files of a message, kept in the store of `rules` (registered with `Feature::files`).
    /// Images show in a fixed box that loads lazily over a placeholder; other files are links
    /// whose downloads go through the store's read permission.
    pub fn attachments(mut self, field: Field<E, Vec<File>>, rules: FileRules) -> Self {
        self.files = Some((field, rules.store()));
        self
    }

    /// Messages written by an AI model: their text is shown as safe Markdown (with the
    /// `markdown` feature; plain text without it), every character escaped, no raw HTML.
    pub fn ai(mut self, field: Field<E, bool>) -> Self {
        self.ai = Some(field);
        self
    }

    /// A heading over the thread, which also names it for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The topics after which the thread fetches new messages, `"chat"` unless set.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        self.topics = topic_list(topics);
        self
    }

    /// Also subscribes to the topics on the realtime hub (feature `realtime`), so a message
    /// another user sends shows up at once. The topics must be declared with `Realtime::topic`.
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }

    /// Streams an AI reply after the user sends a message with a [`Composer`] of the same
    /// conversation: the browser posts the text to `POST /_zeyn/stream` (CSRF, write limit, the
    /// screen's access, the action's permission, interceptors), the action returns a [`Reply`]
    /// and the pieces show as they arrive, as safe Markdown built from text nodes only. Earlier
    /// messages are never rendered again; only the paragraph being written is. The reply is cut
    /// at [`MAX_REPLY_CHARS`] and [`REPLY_SECONDS`], and a stop button ends it. The action
    /// stores the finished message under the reply's key.
    ///
    /// [`Composer`]: crate::ui::Composer
    #[cfg(feature = "server")]
    pub fn reply<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = ReplyRequest, Output = Result<Reply>>,
    {
        self.reply = Some((A::META, start_reply::<A>));
        self
    }

    fn allow(&self, query: &mut DataQuery) {
        let offset = match query.page {
            PageRequest::Offset { offset, .. } => offset,
            PageRequest::After { .. } => 0,
        };
        let mut sort: Vec<Sort> = self
            .when
            .iter()
            .map(|f| Sort {
                field: f.meta().name.to_string(),
                desc: true,
            })
            .collect();
        if let Some(k) = E::fields().iter().find(|f| f.key) {
            sort.push(Sort {
                field: k.name.to_string(),
                desc: true,
            });
        }
        *query = DataQuery {
            page: PageRequest::Offset {
                offset,
                limit: THREAD_PAGE,
            },
            sort,
            with_total: true,
            time_zone: query.time_zone,
            ..DataQuery::default()
        };
    }

    /// The heading of a day: today, yesterday, or the weekday and date.
    fn day_label(r: &Render, day: Date, today: Date) -> String {
        if day == today {
            return r.text(&Text::key("zeyn.ui.chat.today"));
        }
        if today.add_days(-1) == Some(day) {
            return r.text(&Text::key("zeyn.ui.chat.yesterday"));
        }
        format!(
            "{} {}",
            r.text(&Text::key(format!("zeyn.ui.week.days.{}", day.weekday()))),
            escape(&day.format(r.locale))
        )
    }

    fn today(r: &Render) -> Date {
        r.ctx
            .map(|c| c.today())
            .unwrap_or_else(|| r.tz.today(DateTime::now()))
    }

    fn body(&self, out: &mut String, r: &Render, row: &E) {
        let text = self
            .text
            .map(|f| f.get(row).as_str())
            .unwrap_or("");
        let text = clip(text, MAX_MESSAGE_CHARS);
        let ai = self.ai.is_some_and(|f| *f.get(row));
        out.push_str(r#"<div class="z-thread-text" dir="auto">"#);
        #[cfg(feature = "markdown")]
        if ai {
            out.push_str(&super::Markdown::new(text.to_string()).html_in(r));
            out.push_str("</div>");
            return;
        }
        #[cfg(not(feature = "markdown"))]
        let _ = (ai, r);
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                out.push_str("<br>");
            }
            out.push_str(&escape(line.trim_end_matches('\r')));
        }
        out.push_str("</div>");
    }

    fn attachments_html(&self, out: &mut String, r: &Render, row: &E) {
        let Some((field, store)) = self.files else {
            return;
        };
        let files: Vec<&File> = field
            .get(row)
            .iter()
            .filter(|f| valid_file_id(&f.id))
            .collect();
        if files.is_empty() {
            return;
        }
        out.push_str(&format!(
            r#"<ul class="z-thread-files" aria-label="{}">"#,
            r.text(&Text::key("zeyn.ui.chat.attachments"))
        ));
        for f in files.iter().take(MAX_SHOWN_FILES) {
            let url = f.url(store);
            let image = matches!(
                f.content_type.as_str(),
                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
            );
            out.push_str(r#"<li class="z-thread-file">"#);
            if image {
                let img = Image::new(&url)
                    .alt(Text::from(f.name.clone()))
                    .size(IMAGE_BOX.0, IMAGE_BOX.1)
                    .lazy()
                    .placeholder(Placeholder::color("--z-surface-2"))
                    .fit(Fit::Cover)
                    .on_error(Text::from(f.name.clone()));
                img.render(out, r);
            }
            out.push_str(&format!(
                r#"<a class="z-thread-link" href="{}" data-zeyn-base-href download>{}<span class="z-thread-file-name" dir="auto">{}</span> <span class="z-thread-file-size">{}</span></a></li>"#,
                escape(&url),
                Icon::Paperclip.html(),
                escape(&f.name),
                escape(&crate::files::human_size(f.size, r.locale))
            ));
        }
        if files.len() > MAX_SHOWN_FILES {
            out.push_str(&format!(
                r#"<li class="z-thread-file z-thread-more">{}</li>"#,
                r.text(
                    &Text::key("zeyn.ui.chat.more_files")
                        .arg("n", (files.len() - MAX_SHOWN_FILES).to_string())
                )
            ));
        }
        out.push_str("</ul>");
    }

    /// Writes the messages of a page, oldest first. Positions count from the oldest message of
    /// the whole conversation, so they stay put while new messages arrive.
    fn rows(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let rows = page
            .and_then(|p| p.items.downcast_ref::<Vec<E>>())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let offset = page.map_or(0, |p| p.offset);
        let total = page
            .and_then(|p| p.total)
            .unwrap_or(offset + rows.len() as u64);
        let me = r
            .identity
            .and_then(|i| i.user.as_deref());
        let today = Self::today(r);
        let key = E::fields().iter().position(|f| f.key);
        let id = dom_id(r, "thread");
        let mut previous: Option<(&str, i64, Option<Date>)> = None;
        for (i, row) in rows.iter().enumerate().rev() {
            let author = self
                .author
                .map(|f| f.get(row).as_str())
                .unwrap_or("");
            let name = self
                .name
                .map(|f| f.get(row).as_str())
                .filter(|n| !n.is_empty())
                .unwrap_or(author);
            let at = self.when.map(|f| *f.get(row));
            let local = at.map(|a| a.local(r.tz));
            let day = local.map(|l| l.date);
            let ms = at.map_or(0, |a| a.unix_ms());
            let own = me.is_some_and(|m| !author.is_empty() && m == author);
            let new_day = previous.is_none_or(|(_, _, d)| d != day);
            let cont = previous
                .is_some_and(|(a, t, _)| a == author && !new_day && (ms - t).abs() < GROUP_GAP_MS);
            previous = Some((author, ms, day));
            let pos = total
                .saturating_sub(offset + i as u64)
                .max(1);
            let k = key
                .and_then(|k| row.value_at(k))
                .and_then(|v| crate::grid::value_key(&v))
                .unwrap_or_default();
            let ai = self.ai.is_some_and(|f| *f.get(row));
            let label = day
                .map(|d| Self::day_label(r, d, today))
                .unwrap_or_default();
            let mut class = String::from("z-thread-msg");
            class.push_str(if own {
                " z-thread-own"
            } else {
                " z-thread-other"
            });
            if cont {
                class.push_str(" z-thread-cont");
            }
            if ai {
                class.push_str(" z-thread-ai");
            }
            out.push_str(&format!(
                r#"<article class="{class}" tabindex="-1" data-key="{}" data-who="{}" data-at="{ms}" data-day="{}" data-day-label="{}" aria-posinset="{pos}" aria-setsize="{}" aria-labelledby="{id}-{pos}-w {id}-{pos}-t" aria-describedby="{id}-{pos}-b">"#,
                escape(&k),
                short_hash(author),
                day.map(|d| d.to_string())
                    .unwrap_or_default(),
                attr(&label),
                page.and_then(|p| p.total)
                    .map_or(-1, |t| t as i64),
            ));
            if new_day {
                out.push_str(&format!(
                    r#"<h3 class="z-thread-day"><span>{label}</span></h3>"#
                ));
            }
            let who = if own {
                r.text(&Text::key("zeyn.ui.chat.you"))
            } else {
                escape(name)
            };
            out.push_str(&format!(
                r#"<p class="z-thread-who" id="{id}-{pos}-w" dir="auto">{who}</p><div class="z-thread-bubble" id="{id}-{pos}-b">"#
            ));
            self.body(out, r, row);
            self.attachments_html(out, r, row);
            out.push_str(r#"<p class="z-thread-meta">"#);
            match (at, local) {
                (Some(a), Some(l)) => out.push_str(&format!(
                    r#"<time id="{id}-{pos}-t" datetime="{a}">{}</time>"#,
                    escape(
                        &crate::time::Time::new(l.time.hour(), l.time.minute(), 0)
                            .unwrap_or_default()
                            .format(r.locale)
                    )
                )),
                _ => out.push_str(&format!(r#"<span id="{id}-{pos}-t"></span>"#)),
            }
            if let (true, Some(f)) = (own, self.status) {
                let s = *f.get(row);
                out.push_str(&format!(
                    r#"<span class="z-thread-status z-thread-status-{code}">{}<span class="z-sr-only">{}</span></span>"#,
                    s.icon().html(),
                    r.text(&Text::key(format!("zeyn.ui.chat.status.{}", s.code()))),
                    code = s.code(),
                ));
            }
            out.push_str("</p></div></article>");
        }
    }

    fn texts(&self, r: &Render) -> Json {
        let t = |k: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.chat.{k}")))
        };
        let today = Self::today(r);
        json!({
            "new": t("new_message"),
            "assistant": t("assistant"),
            "writing": t("writing"),
            "stop": t("stop"),
            "stopped": t("stopped"),
            "failed": t("reply_failed"),
            "loading": t("loading"),
            "today": t("today"),
            "todayDate": today.to_string(),
        })
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let id = dom_id(r, "thread");
        let (title, labelled) = match &self.title {
            Some(title) => (
                format!(
                    r#"<h2 class="z-thread-title" id="{id}-title">{}</h2>"#,
                    r.text(title)
                ),
                format!(r#" aria-labelledby="{id}-title""#),
            ),
            None => (
                String::new(),
                format!(
                    r#" aria-label="{}""#,
                    r.text(&Text::key("zeyn.ui.chat.label"))
                ),
            ),
        };
        let topics = self.topics.join(",");
        let live = if self.live && !topics.is_empty() {
            format!(
                r#" data-zeyn-live="{}" data-zeyn-live-self"#,
                escape(&topics)
            )
        } else {
            String::new()
        };
        #[cfg(feature = "server")]
        let reply = self
            .reply
            .as_ref()
            .filter(|(meta, _)| r.can(&meta.access))
            .map_or("", |_| " data-zeyn-thread-reply");
        #[cfg(not(feature = "server"))]
        let reply = "";
        let count = page.map_or(0, |p| p.count);
        let total = page
            .and_then(|p| p.total)
            .unwrap_or(count as u64);
        out.push_str(&format!(
            r#"<section class="z-thread" data-zeyn-thread data-room="{room}" data-topics="{topics}" data-page="{THREAD_PAGE}" data-count="{count}" data-total="{total}" data-texts="{texts}"{reply}{live} data-testid="{scope}.thread">{title}<div class="z-thread-view" role="feed" tabindex="0"{labelled} aria-busy="false">"#,
            room = escape(&self.conversation),
            topics = escape(&topics),
            texts = attr(&self.texts(r)),
            scope = r.scope,
        ));
        if count == 0 {
            out.push_str(&format!(
                r#"<p class="z-thread-empty">{}</p>"#,
                r.text(&Text::key("zeyn.ui.chat.empty"))
            ));
        }
        self.rows(out, r, page);
        out.push_str(
            r#"</div><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-thread-status></p></section>"#,
        );
    }
}

impl<E: Model> GridSource for MessageThread<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self, _view: Option<&crate::grid::DataGridView>) -> DataQuery {
        let mut query = DataQuery::default();
        self.allow(&mut query);
        query
    }

    fn clean_view(&self, _view: crate::grid::DataGridView) -> crate::grid::DataGridView {
        crate::grid::DataGridView::default()
    }

    fn head_html(&self, _r: &Render, _view: Option<&crate::grid::DataGridView>) -> String {
        String::new()
    }

    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        self.allow(&mut query);
        query.time_zone = ctx.time_zone();
        let offset = match query.page {
            PageRequest::Offset { offset, .. } => offset,
            PageRequest::After { .. } => 0,
        };
        let fetch = self.fetch;
        let conversation = self.conversation.clone();
        Box::pin(async move {
            check_masks::<E>(&ctx)?;
            let mut page = fetch(
                ctx,
                ThreadQuery {
                    conversation,
                    query,
                },
            )
            .await?;
            page.items
                .truncate(THREAD_PAGE as usize);
            Ok(LoadedPage {
                count: page.items.len(),
                total: page.total,
                next: None,
                offset,
                totals: Vec::new(),
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        let mut out = String::new();
        self.rows(&mut out, r, Some(page));
        out
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "description": "Messages of the conversation, newest first. The server sets the query." })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for MessageThread<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        #[cfg(feature = "server")]
        let reply = self
            .reply
            .as_ref()
            .map(|(meta, _)| *meta);
        #[cfg(not(feature = "server"))]
        let reply: Option<ActionMeta> = None;
        json!({
            "type": "message_thread",
            "model": E::NAME,
            "action": self.action,
            "conversation": self.conversation,
            "title": self.title,
            "reply": reply,
            "topics": self.topics,
            "live": self.live,
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }

    #[cfg(feature = "server")]
    fn stream(&self, key: &str) -> Option<&dyn StreamSource> {
        match (key, &self.reply) {
            ("reply", Some(_)) => Some(self),
            _ => None,
        }
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
        if self.author.is_none() || self.text.is_none() || self.when.is_none() {
            check.problem("a MessageThread needs `.author(field)`, `.text(field)` and `.when(field)`: who sent a message, its text and when");
        }
        if !E::fields().iter().any(|f| f.key) {
            check.problem("a MessageThread needs a model with a `#[key]` field, so messages that load twice show once");
        }
    }
}

/// The reply of [`MessageThread::reply`]: reads `{ "text": ... }` from the browser and adds the
/// thread's conversation.
#[cfg(feature = "server")]
impl<E: Model> StreamSource for MessageThread<E> {
    fn action(&self) -> ActionMeta {
        self.reply
            .map_or(self.action, |(meta, _)| meta)
    }

    fn start(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Reply>> {
        let Some((_, run)) = self.reply else {
            return Box::pin(async { Err(Error::NotFound) });
        };
        let text = input
            .get("text")
            .and_then(Json::as_str)
            .map(str::trim)
            .unwrap_or("");
        if text.is_empty() || text.chars().count() > MAX_MESSAGE_CHARS {
            return Box::pin(async { Err(Error::NotFound) });
        }
        run(
            ctx,
            ReplyRequest {
                conversation: self.conversation.clone(),
                text: text.to_string(),
            },
        )
    }
}

impl<E: Model> From<MessageThread<E>> for Node {
    fn from(t: MessageThread<E>) -> Self {
        Node::new(t)
    }
}
