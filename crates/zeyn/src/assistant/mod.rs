//! Embedded AI assistant: a chat that drives the app's screens and calls its APIs as the signed in
//! user.
//!
//! Turn it on with [`crate::App::assistant`]. Everything is pluggable: the language model
//! ([`LlmProvider`]), speech ([`SpeechToText`], [`TextToSpeech`]) and where the chat history is
//! kept ([`ChatStore`]). The tools the model sees are the same ones MCP and WebMCP offer, filtered
//! for the user; every tool call runs through the normal action path. See `docs/assistant.md`.

pub(crate) mod chat;
mod http;
mod script;

pub use chat::AssistantChat;
pub use http::{Anthropic, OpenAiCompatible, OpenAiSpeech, OpenAiTranscription};
pub use script::{Say, ScriptedProvider};

use crate::ctx::BoxFuture;
use crate::i18n::Text;
use crate::screen::ScreenDef;
use crate::security::RateLimit;
use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Who wrote a chat message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    /// The result of a tool call, written by the server.
    Tool,
}

/// A tool call the model asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Json,
}

/// One message of a conversation, as stored in a [`ChatStore`] and sent to providers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    #[serde(default)]
    pub text: String,
    /// Tools the model called in this message (assistant messages).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// The call this message answers (tool messages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Provider content to send back unchanged within the same turn, such as the reasoning
    /// blocks of a model that needs them with tool results. Providers drop it for older turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Json>,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::User,
            text: text.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            raw: None,
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::Assistant,
            text: text.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            raw: None,
        }
    }

    pub fn tool(call_id: impl Into<String>, text: impl Into<String>) -> Self {
        ChatMessage {
            role: Role::Tool,
            text: text.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(call_id.into()),
            raw: None,
        }
    }
}

/// A tool as the model sees it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments.
    pub schema: Json,
}

/// One request to a language model.
#[derive(Clone, Debug)]
pub struct ChatRequest {
    /// Instructions built by the server for every round. Never contains user or tool data.
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
    pub max_tokens: u32,
}

/// What a model streams back.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    /// A piece of the answer.
    Text(String),
    /// A complete tool call.
    ToolCall(ToolCall),
    /// Tokens used by the round, for the user's token limit.
    Usage { input: u32, output: u32 },
    /// Provider content to keep with the message (see [`ChatMessage::raw`]).
    Raw(Json),
}

/// Events of one model round, through a bounded channel. Dropping the receiver stops the
/// provider's request.
pub type ChatStream = tokio::sync::mpsc::Receiver<crate::Result<ChatEvent>>;

/// Capacity of the channels that carry streamed events and bytes.
pub const STREAM_BUFFER: usize = 32;

/// A language model with streaming chat completions and tool calls.
///
/// Built in: [`OpenAiCompatible`] (OpenAI, Azure OpenAI, Ollama, vLLM, LiteLLM and other
/// compatible servers), [`Anthropic`] and [`ScriptedProvider`] for tests and demos.
pub trait LlmProvider: Send + Sync + 'static {
    /// Starts one round. Events arrive on the returned channel as the model produces them.
    fn chat(&self, request: ChatRequest) -> BoxFuture<crate::Result<ChatStream>>;
}

/// Recorded audio to turn into text.
#[derive(Clone, Debug)]
pub struct Audio {
    pub bytes: Bytes,
    /// Media type such as `audio/webm`.
    pub mime: String,
}

/// Speech to text on the server. Without one, the chat uses the browser's own speech
/// recognition (Web Speech API), which costs the server nothing.
pub trait SpeechToText: Send + Sync + 'static {
    /// The words spoken in `audio`. `locale` is the user's language, such as `tr`.
    fn transcribe(&self, audio: Audio, locale: String) -> BoxFuture<crate::Result<String>>;
}

/// Audio streamed from a [`TextToSpeech`] provider.
pub struct SpeechStream {
    /// Media type such as `audio/mpeg`.
    pub mime: String,
    pub chunks: tokio::sync::mpsc::Receiver<crate::Result<Bytes>>,
}

/// Text to speech on the server. Without one, "read aloud" uses the browser's speech synthesis.
pub trait TextToSpeech: Send + Sync + 'static {
    fn speak(&self, text: String, locale: String) -> BoxFuture<crate::Result<SpeechStream>>;
}

/// Whose conversation: tenants never share history.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChatKey {
    /// Tenant id, empty without tenants.
    pub tenant: String,
    pub user: String,
}

/// Where conversations are kept. The default is [`MemoryChatStore`]; implement it to keep
/// history in a database.
pub trait ChatStore: Send + Sync + 'static {
    /// The last `limit` messages of the conversation, oldest first.
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<crate::Result<Vec<ChatMessage>>>;

    /// Adds messages to the end of the conversation.
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<crate::Result<()>>;

    /// Deletes the conversation.
    fn clear(&self, key: &ChatKey) -> BoxFuture<crate::Result<()>>;
}

/// Conversations in memory, capped twice: at most `max_users` conversations (the least recently
/// used one goes first) of at most `max_messages` messages each. Lost on restart.
pub struct MemoryChatStore {
    chats: Mutex<HashMap<ChatKey, (u64, VecDeque<ChatMessage>)>>,
    max_users: usize,
    max_messages: usize,
}

impl MemoryChatStore {
    pub fn new(max_users: usize, max_messages: usize) -> Self {
        MemoryChatStore {
            chats: Mutex::new(HashMap::new()),
            max_users: max_users.max(1),
            max_messages: max_messages.max(2),
        }
    }

    /// Number of conversations held.
    pub fn len(&self) -> usize {
        self.chats
            .lock()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for MemoryChatStore {
    /// 1,000 conversations of 60 messages.
    fn default() -> Self {
        MemoryChatStore::new(1_000, 60)
    }
}

static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl ChatStore for MemoryChatStore {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<crate::Result<Vec<ChatMessage>>> {
        let list = self
            .chats
            .lock()
            .ok()
            .and_then(|c| {
                c.get(key).map(|(_, m)| {
                    m.iter()
                        .skip(m.len().saturating_sub(limit))
                        .cloned()
                        .collect()
                })
            })
            .unwrap_or_default();
        Box::pin(async move { Ok(list) })
    }

    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<crate::Result<()>> {
        let tick = TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut chats) = self.chats.lock() {
            if !chats.contains_key(key) && chats.len() >= self.max_users {
                let oldest = chats
                    .iter()
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(k, _)| k.clone());
                if let Some(k) = oldest {
                    chats.remove(&k);
                }
            }
            let (at, list) = chats.entry(key.clone()).or_default();
            *at = tick;
            list.extend(messages);
            while list.len() > self.max_messages {
                list.pop_front();
            }
        }
        Box::pin(async { Ok(()) })
    }

    fn clear(&self, key: &ChatKey) -> BoxFuture<crate::Result<()>> {
        if let Ok(mut chats) = self.chats.lock() {
            chats.remove(key);
        }
        Box::pin(async { Ok(()) })
    }
}

/// Size and cost limits of the assistant. Every one is enforced on the server.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Longest user message in characters.
    pub message_chars: usize,
    /// Most messages sent to the model per round.
    pub history: usize,
    /// Most model rounds (tool calls and answers) for one user message.
    pub rounds: usize,
    /// Most tokens one model answer may have.
    pub output_tokens: u32,
    /// Longest tool result given to the model, in characters.
    pub tool_result_chars: usize,
    /// Largest recording sent to speech to text, in bytes.
    pub audio_bytes: usize,
    /// Longest text sent to text to speech, in characters.
    pub speech_chars: usize,
    /// Longest wait for the next piece of a model answer.
    pub idle: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            message_chars: 2_000,
            history: 30,
            rounds: 6,
            output_tokens: 1_024,
            tool_result_chars: 6_000,
            audio_bytes: MAX_AUDIO_BYTES,
            speech_chars: 2_000,
            idle: Duration::from_secs(60),
        }
    }
}

/// Hard cap on a speech to text upload, whatever the limits say.
pub const MAX_AUDIO_BYTES: usize = 4 * 1024 * 1024;

/// The embedded assistant: its model, speech, history and limits.
///
/// ```ignore
/// let model = OpenAiCompatible::new("https://api.openai.com/v1", "gpt-4.1-mini")
///     .api_key(config.secret("assistant.api_key")?);
/// App::new("Lifts")
///     .assistant(
///         Assistant::new(model)
///             .permission("lifts.assistant.use")
///             .instructions("You help service engineers plan elevator maintenance.")
///             .suggest(work_orders, "Which lifts are overdue for service?"),
///     )
/// ```
pub struct Assistant {
    pub(crate) provider: Arc<dyn LlmProvider>,
    pub(crate) stt: Option<Arc<dyn SpeechToText>>,
    pub(crate) tts: Option<Arc<dyn TextToSpeech>>,
    pub(crate) voice: bool,
    pub(crate) store: Arc<dyn ChatStore>,
    pub(crate) permission: Option<&'static str>,
    pub(crate) instructions: String,
    pub(crate) suggestions: Vec<(&'static str, Text)>,
    pub(crate) launcher: bool,
    pub(crate) writes: bool,
    pub(crate) limits: Limits,
    pub(crate) rate: RateLimit,
    pub(crate) tokens: Option<RateLimit>,
}

impl Assistant {
    /// An assistant on `provider`, with the browser's speech, [`MemoryChatStore`], a floating
    /// launcher on every page, 20 messages a minute and 200,000 tokens an hour per user.
    pub fn new(provider: impl LlmProvider) -> Self {
        Assistant {
            provider: Arc::new(provider),
            stt: None,
            tts: None,
            voice: true,
            store: Arc::new(MemoryChatStore::default()),
            permission: None,
            instructions: String::new(),
            suggestions: Vec::new(),
            launcher: true,
            writes: true,
            limits: Limits::default(),
            rate: RateLimit::per_user(20, Duration::from_secs(60)),
            tokens: Some(RateLimit::per_user(200_000, Duration::from_secs(3_600))),
        }
    }

    /// Speech to text on the server instead of the browser.
    pub fn speech_to_text(mut self, stt: impl SpeechToText) -> Self {
        self.stt = Some(Arc::new(stt));
        self
    }

    /// Text to speech on the server instead of the browser.
    pub fn text_to_speech(mut self, tts: impl TextToSpeech) -> Self {
        self.tts = Some(Arc::new(tts));
        self
    }

    /// No microphone and no read aloud.
    pub fn no_voice(mut self) -> Self {
        self.voice = false;
        self
    }

    /// Where conversations are kept.
    pub fn store(mut self, store: impl ChatStore) -> Self {
        self.store = Arc::new(store);
        self
    }

    /// Only users with this permission see and use the assistant. Without it every signed in
    /// user may.
    pub fn permission(mut self, permission: &'static str) -> Self {
        self.permission = Some(permission);
        self
    }

    /// What the app tells the model about itself, its users and its domain. Written by the
    /// app's developers; user and tool data never go here.
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = text.into();
        self
    }

    /// A starter prompt shown as a chip while the user is on `screen`.
    pub fn suggest<S: ScreenDef>(mut self, _screen: S, prompt: impl Into<Text>) -> Self {
        self.suggestions
            .push((S::META.name, prompt.into()));
        self
    }

    /// A starter prompt shown on every screen.
    pub fn suggest_anywhere(mut self, prompt: impl Into<Text>) -> Self {
        self.suggestions
            .push(("", prompt.into()));
        self
    }

    /// `false` leaves out the floating launcher; the chat is then only where a screen places
    /// [`AssistantChat`].
    pub fn launcher(mut self, on: bool) -> Self {
        self.launcher = on;
        self
    }

    /// The assistant may search and navigate but never save or change anything.
    pub fn read_only(mut self) -> Self {
        self.writes = false;
        self
    }

    /// Size and cost limits.
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = Limits {
            audio_bytes: limits.audio_bytes.min(MAX_AUDIO_BYTES),
            ..limits
        };
        self
    }

    /// How many messages a user may send (chat, confirmations, speech).
    pub fn rate_limit(mut self, limit: RateLimit) -> Self {
        self.rate = limit;
        self
    }

    /// How many model tokens (input and output) a user may use per window; `None` for no limit.
    pub fn token_limit(mut self, limit: Option<RateLimit>) -> Self {
        self.tokens = limit;
        self
    }
}

/// Cuts `text` to at most `max` characters on a character boundary.
pub(crate) fn clip(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}
