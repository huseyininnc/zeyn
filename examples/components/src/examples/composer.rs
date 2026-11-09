//! Composer examples: a chat with a conversation list, a thread and a composer with attachments,
//! mentions and drafts, and an assistant style thread whose replies stream from a local fake
//! generator. Messages go to the shared sample store of `message_thread`, which keeps at most
//! `MAX_SAMPLE_MESSAGES`; uploads go to a memory store that keeps the last `MAX_SAMPLE_FILES`.

use crate::examples::message_thread::{ChatMessage, ChatMessageId, add, thread};
use crate::{Demo, Entry, exd};
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use zeyn::prelude::*;
use zeyn::ui::{
    ChatSend, Composer, Delivery, LookupItem, LookupQuery, Reply, ReplyRequest, SendKey,
};
use zeyn::{ClientPref, Draft, Purpose};
use zeyn::{FileChunks, FileMeta, FileStore, MemoryStore, StoredFile};

/// Uploads of the chat examples: images and PDFs up to 1 MB, 4 per message.
pub const CHAT_FILES: FileRules = FileRules::new("chat-demo")
    .max_bytes(1024 * 1024)
    .max_files(4)
    .accept(&[FileType::Png, FileType::Jpeg, FileType::Webp, FileType::Pdf]);

/// Most uploads the shared memory store keeps; the oldest goes first.
pub const MAX_SAMPLE_FILES: usize = 20;

/// Unsent messages, kept per conversation for the session.
pub const CHAT_DRAFT: ClientPref<Draft> = ClientPref::draft(
    "chat-draft",
    Duration::from_secs(86_400),
    Purpose::Preference,
);

/// The conversations of the list: id and name.
const ROOMS: [(&str, &str); 2] = [("team", "Team"), ("support", "Support")];

/// The people mentions find.
const PEOPLE: [(i64, &str, &str); 4] = [
    (1, "Ada Lovelace", "Sales"),
    (2, "Grace Hopper", "Warehouse"),
    (3, "Alan Turing", "Support"),
    (4, "Katherine Johnson", "Finance"),
];

/// A memory store that drops its oldest file when it holds `MAX_SAMPLE_FILES`.
#[derive(Clone)]
pub struct SampleFiles {
    inner: MemoryStore,
    order: Arc<Mutex<VecDeque<String>>>,
}

impl SampleFiles {
    pub fn new() -> Self {
        SampleFiles {
            inner: MemoryStore::new(MAX_SAMPLE_FILES as u64 * 1024 * 1024 + 1),
            order: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// How many uploads the store holds now.
    pub fn stored(&self) -> usize {
        self.inner.ids().len()
    }
}

impl Default for SampleFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl FileStore for SampleFiles {
    fn put(&self, meta: FileMeta, chunks: FileChunks) -> zeyn::BoxFuture<Result<()>> {
        let gone: Vec<String> = {
            let mut order = self
                .order
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            order.push_back(meta.id.clone());
            let extra = order
                .len()
                .saturating_sub(MAX_SAMPLE_FILES);
            order.drain(..extra).collect()
        };
        let inner = self.inner.clone();
        Box::pin(async move {
            for id in gone {
                inner.delete(id).await?;
            }
            inner.put(meta, chunks).await
        })
    }

    fn get(&self, id: String) -> zeyn::BoxFuture<Result<StoredFile>> {
        self.inner.get(id)
    }

    fn meta(&self, id: String) -> zeyn::BoxFuture<Result<FileMeta>> {
        self.inner.meta(id)
    }

    fn delete(&self, id: String) -> zeyn::BoxFuture<Result<()>> {
        self.inner.delete(id)
    }
}

/// The shared upload store of the chat examples.
pub static FILES: LazyLock<SampleFiles> = LazyLock::new(SampleFiles::new);

/// Stores a message of the visitor in the conversation.
#[action(public)]
pub async fn send_sample(m: ChatSend) -> Result<()> {
    let known = ROOMS
        .iter()
        .any(|(id, _)| *id == m.conversation)
        || m.conversation == "assistant";
    if !known {
        return Err(Error::NotFound);
    }
    let mut body = m.text;
    for f in &m.attachments {
        body.push_str(&format!("\n[{}]", f.name));
    }
    add(ChatMessage {
        id: ChatMessageId(0),
        room: m.conversation,
        author: "visitor".into(),
        author_name: "You".into(),
        body,
        sent_at: DateTime::now(),
        status: Delivery::Delivered,
        ai: false,
    });
    Ok(())
}

/// People whose name has the text, for mentions.
#[action(public)]
pub async fn find_people(q: LookupQuery) -> Result<Page<LookupItem>> {
    let text = zeyn::text::fold(&q.text);
    let items: Vec<LookupItem> = PEOPLE
        .iter()
        .filter(|(_, name, _)| zeyn::text::fold(name).contains(&text))
        .take(q.limit() as usize)
        .map(|(id, name, team)| LookupItem::new(*id, *name).hint(*team))
        .collect();
    Ok(Page::new(items))
}

/// The made up answer of the fake assistant: what it was asked, then some Markdown.
fn answer(question: &str) -> String {
    let topic: String = question.chars().take(80).collect();
    format!(
        "You asked about **{topic}**. This answer is written by a local generator, word by word, to show how a streamed reply arrives.\n\nWhat happens:\n\n- Each piece is sent as a JSON line\n- Only the paragraph being written is drawn again\n- The screen reader hears the answer once, when it is finished\n\nCode stays as text: `<b>not bold</b>`.\n\n1. Nothing here calls another service\n2. The finished answer is stored like any message"
    )
}

/// A made up answer of 2,000 words in 40 paragraphs, for a question with "long" in it.
fn long_answer() -> String {
    (1..=40)
        .map(|p| {
            let words: Vec<String> = (1..=50)
                .map(|w| format!("w{}", p * 100 + w))
                .collect();
            format!("Paragraph {p}: {}", words[1..].join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Streams the fake assistant's answer word by word and stores it when it is finished.
#[action(public)]
pub async fn sample_reply(r: ReplyRequest) -> Result<Reply> {
    if r.conversation != "assistant" {
        return Err(Error::NotFound);
    }
    let id = add(ChatMessage {
        id: ChatMessageId(0),
        room: "assistant-pending".into(),
        author: "assistant".into(),
        author_name: "Assistant".into(),
        body: String::new(),
        sent_at: DateTime::now(),
        status: Delivery::Read,
        ai: true,
    });
    let (tx, reply) = Reply::channel(id.to_string());
    let long = r.text.to_lowercase().contains("long");
    let text = match long {
        true => long_answer(),
        false => answer(&r.text),
    };
    let pause = Duration::from_millis(if long { 4 } else { 30 });
    tokio::spawn(async move {
        let mut written = String::new();
        for word in text.split_inclusive(' ') {
            tokio::time::sleep(pause).await;
            written.push_str(word);
            if !tx.send(word).await {
                break;
            }
        }
        let mut all = crate::examples::message_thread::messages();
        if let Some(m) = all.iter_mut().find(|m| m.id.0 == id) {
            m.room = "assistant".into();
            m.body = written;
            m.sent_at = DateTime::now();
        }
    });
    Ok(reply)
}

/// The conversation shown: the one in the params, the first one otherwise.
fn room_of(demo: &Demo) -> &'static str {
    ROOMS
        .iter()
        .find(|(id, _)| demo.room.as_deref() == Some(*id))
        .map_or(ROOMS[0].0, |(id, _)| *id)
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let room = room_of(demo);
    let mut list = Stack::new();
    for (id, name) in ROOMS {
        let params = Demo {
            room: Some(id.to_string()),
            ..Demo::default()
        };
        let link = LinkButton::new(name, crate::composer, params);
        list = list.add(if id == room { link.primary() } else { link });
    }
    Row::new()
        .col(list)
        .col(
            Stack::new()
                .add(
                    thread(room).title(
                        ROOMS
                            .iter()
                            .find(|(id, _)| *id == room)
                            .map_or("", |(_, n)| *n),
                    ),
                )
                .add(
                    Composer::of(send_sample)
                        .conversation(room)
                        .attachments(CHAT_FILES)
                        .mentions(find_people)
                        .remember(CHAT_DRAFT),
                ),
        )
        .into()
}

pub fn assistant(_ctx: &Ctx, _demo: &Demo) -> Node {
    Stack::new()
        .add(
            thread("assistant")
                .title("Assistant")
                .reply(sample_reply),
        )
        .add(
            Composer::of(send_sample)
                .conversation("assistant")
                .send_key(SendKey::CtrlEnter)
                .placeholder("Ask the assistant"),
        )
        .into()
}

/// What the catalog shows of `Composer`.
pub const ENTRY: Entry = Entry {
    name: "Composer",
    docs: "components/chat#composer",
    api: &["Composer"],
    overview: &[exd!(overview)],
    examples: &[exd!(assistant)],
};
