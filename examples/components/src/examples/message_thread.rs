//! MessageThread examples: a conversation with grouping, day headings and delivery states, an AI
//! answer shown as Markdown, and a history of 5,000 messages that loads as it scrolls. The sample
//! messages are shared by every visitor; the store keeps at most `MAX_SAMPLE_MESSAGES`.

use crate::{Entry, ex};
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;
use zeyn::ui::{Delivery, MessageThread, ThreadQuery};

/// A message of the sample conversations.
#[entity(table = "chat_messages")]
pub struct ChatMessage {
    #[key]
    pub id: ChatMessageId,
    pub room: String,
    pub author: String,
    pub author_name: String,
    pub body: String,
    pub sent_at: DateTime,
    pub status: Delivery,
    pub ai: bool,
}

/// Most messages the shared store holds; the oldest go first.
pub const MAX_SAMPLE_MESSAGES: usize = 300;

/// Messages in the long history example, made up on the fly and never stored.
pub const LONG_HISTORY: u64 = 5_000;

/// The conversation the long history example shows.
pub const HISTORY_ROOM: &str = "history";

const MINUTE: i64 = 60_000;

/// The sample conversations, relative to when the catalog started.
fn sample() -> Vec<ChatMessage> {
    let now = DateTime::now();
    let rows = [
        (
            "team",
            "ada",
            "Ada",
            "Morning! Did the new price list go out?",
            26 * 60,
            false,
        ),
        (
            "team",
            "visitor",
            "You",
            "Yes, at 9:00. Customers get it with the next invoice.",
            26 * 60 - 3,
            false,
        ),
        (
            "team",
            "ada",
            "Ada",
            "Great. Can you check the Green tea stock?",
            50,
            false,
        ),
        (
            "team",
            "ada",
            "Ada",
            "Warehouse A says 12 units.",
            49,
            false,
        ),
        (
            "team",
            "visitor",
            "You",
            "Ordered 200 more, arriving Thursday.",
            20,
            false,
        ),
        (
            "team",
            "grace",
            "Grace",
            "I will take the delivery.\nPlease put the order number in the ticket.",
            5,
            false,
        ),
        (
            "assistant",
            "visitor",
            "You",
            "How do I export the open orders?",
            8,
            false,
        ),
        (
            "assistant",
            "assistant",
            "Assistant",
            "Open **Orders**, then:\n\n1. Filter by *Status: open*\n2. Press `Export`\n3. Pick CSV or XLSX\n\nThe file holds the columns you see.",
            7,
            true,
        ),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (room, author, name, body, minutes, ai))| ChatMessage {
            id: ChatMessageId(i as i64 + 1),
            room: room.to_string(),
            author: author.to_string(),
            author_name: name.to_string(),
            body: body.to_string(),
            sent_at: now.add_ms(-i64::from(*minutes) * MINUTE),
            status: Delivery::Read,
            ai: *ai,
        })
        .collect()
}

static MESSAGES: LazyLock<Mutex<Vec<ChatMessage>>> = LazyLock::new(|| Mutex::new(sample()));

pub(crate) fn messages() -> std::sync::MutexGuard<'static, Vec<ChatMessage>> {
    MESSAGES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// How many messages the shared store holds now.
pub fn stored() -> usize {
    messages().len()
}

/// Adds a message to the shared store, dropping the oldest beyond the cap, and returns its id.
pub fn add(mut message: ChatMessage) -> i64 {
    let mut all = messages();
    let id = all
        .iter()
        .map(|m| m.id.0)
        .max()
        .unwrap_or(0)
        + 1;
    message.id = ChatMessageId(id);
    all.push(message);
    let extra = all
        .len()
        .saturating_sub(MAX_SAMPLE_MESSAGES);
    all.drain(..extra);
    id
}

/// The messages of a long made up history, newest first, only the page asked for.
fn long_history(q: &DataQuery) -> Page<ChatMessage> {
    let zeyn::Span { offset, limit } = q.span();
    let start = DateTime::now().add_ms(-(LONG_HISTORY as i64) * 7 * MINUTE);
    let items = (offset..(offset + u64::from(limit)).min(LONG_HISTORY))
        .map(|i| {
            let n = LONG_HISTORY - 1 - i;
            let own = (n / 3).is_multiple_of(2);
            ChatMessage {
                id: ChatMessageId(n as i64 + 1),
                room: HISTORY_ROOM.into(),
                author: if own { "visitor" } else { "ada" }.into(),
                author_name: if own { "You" } else { "Ada" }.into(),
                body: format!("Message {} of the long history", n + 1),
                sent_at: start.add_ms(n as i64 * 7 * MINUTE),
                status: Delivery::Read,
                ai: false,
            }
        })
        .collect();
    Page::new(items).total(LONG_HISTORY)
}

/// The messages of a sample conversation.
#[action(public)]
pub async fn sample_thread(q: ThreadQuery) -> Result<Page<ChatMessage>> {
    if q.conversation == HISTORY_ROOM {
        return Ok(long_history(&q.query));
    }
    let rows: Vec<ChatMessage> = messages()
        .iter()
        .filter(|m| m.room == q.conversation)
        .cloned()
        .collect();
    Ok(q.query.apply(rows))
}

/// A thread of `room` with the fields of the sample messages.
pub fn thread(room: &str) -> MessageThread<ChatMessage> {
    MessageThread::of(sample_thread)
        .conversation(room)
        .author(ChatMessage::author)
        .name(ChatMessage::author_name)
        .text(ChatMessage::body)
        .when(ChatMessage::sent_at)
        .status(ChatMessage::status)
        .ai(ChatMessage::ai)
}

pub fn overview(_ctx: &Ctx) -> Node {
    thread("team").title("Team").into()
}

pub fn ai_answer(_ctx: &Ctx) -> Node {
    thread("assistant")
        .title("Assistant")
        .into()
}

pub fn long(_ctx: &Ctx) -> Node {
    thread(HISTORY_ROOM)
        .title("5,000 messages")
        .into()
}

/// What the catalog shows of `MessageThread`.
pub const ENTRY: Entry = Entry {
    name: "MessageThread",
    docs: "components/chat#messagethread",
    api: &["MessageThread", "Reply"],
    overview: &[ex!(overview)],
    examples: &[ex!(ai_answer), ex!(long)],
};
