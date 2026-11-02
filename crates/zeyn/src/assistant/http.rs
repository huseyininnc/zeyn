//! Built in providers over HTTP: OpenAI compatible chat, transcription and speech, and the
//! Anthropic Messages API. Answers are read as a stream and passed on piece by piece through
//! bounded channels; nothing is buffered whole. API keys stay on the server and never appear in
//! errors or logs.

use super::{
    Audio, ChatEvent, ChatMessage, ChatRequest, ChatStream, LlmProvider, Role, STREAM_BUFFER, SpeechStream,
    SpeechToText, TextToSpeech, ToolCall,
};
use crate::action::Error;
use crate::ctx::BoxFuture;
use crate::platform::Secret;
use axum::body::Bytes;
use serde_json::{Map, Value as Json, json};
use std::time::Duration;
use tokio::sync::mpsc::{Sender, channel};

/// Longest line of a streamed answer; a provider sending more is cut off.
const MAX_LINE: usize = 1024 * 1024;

/// Most bytes of an error body kept for the server log.
const ERROR_BODY: usize = 2_048;

/// HTTP client shared by the providers of one app, with TLS from rustls and the platform's
/// certificate store.
fn client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(120))
        .build()
        .unwrap_or_default()
}

/// How a request proves who it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyHeader {
    Bearer,
    /// Azure OpenAI's `api-key` header.
    ApiKey,
}

fn authorize(req: reqwest::RequestBuilder, key: Option<&Secret<String>>, header: KeyHeader) -> reqwest::RequestBuilder {
    match (key, header) {
        (None, _) => req,
        (Some(k), KeyHeader::Bearer) => req.bearer_auth(k.expose()),
        (Some(k), KeyHeader::ApiKey) => req.header("api-key", k.expose()),
    }
}

/// Sends a request and returns the response when it succeeded; otherwise an internal error with
/// the status and the start of the body, for the server log only.
async fn send(req: reqwest::RequestBuilder, what: &str) -> crate::Result<reqwest::Response> {
    let res = req.send().await.map_err(|e| Error::Internal(format!("{what}: request failed: {}", e.without_url())))?;
    if res.status().is_success() {
        return Ok(res);
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(Error::Internal(format!("{what}: {status}: {}", super::clip(&body, ERROR_BODY))))
}

/// Reads a streamed body as server sent events, one data line at a time, with a buffer that
/// holds at most one line.
struct Sse {
    res: reqwest::Response,
    buf: Vec<u8>,
    event: String,
}

impl Sse {
    fn new(res: reqwest::Response) -> Self {
        Sse { res, buf: Vec::with_capacity(4096), event: String::new() }
    }

    /// The next event name and data line; `None` at the end of the body.
    async fn next(&mut self) -> crate::Result<Option<(String, String)>> {
        loop {
            while let Some(end) = self.buf.iter().position(|b| *b == b'\n') {
                let raw: Vec<u8> = self.buf.drain(..=end).collect();
                let line = String::from_utf8_lossy(&raw);
                let line = line.trim_end_matches(['\n', '\r']);
                if let Some(name) = line.strip_prefix("event:") {
                    self.event = name.trim().to_string();
                } else if let Some(data) = line.strip_prefix("data:") {
                    return Ok(Some((self.event.clone(), data.trim_start().to_string())));
                } else if line.is_empty() {
                    self.event.clear();
                }
            }
            if self.buf.len() > MAX_LINE {
                return Err(Error::Internal("model stream line too long".into()));
            }
            let chunk = self
                .res
                .chunk()
                .await
                .map_err(|e| Error::Internal(format!("model stream broke: {}", e.without_url())))?;
            match chunk {
                Some(c) => self.buf.extend_from_slice(&c),
                None => return Ok(None),
            }
        }
    }
}

/// Sends events in order; `false` when the receiver is gone (the user stopped).
async fn forward(tx: &Sender<crate::Result<ChatEvent>>, events: &mut Vec<ChatEvent>) -> bool {
    for e in events.drain(..) {
        if tx.send(Ok(e)).await.is_err() {
            return false;
        }
    }
    true
}

/// Runs `work` in a task that streams events through a bounded channel.
fn spawn_stream<F>(work: impl FnOnce(Sender<crate::Result<ChatEvent>>) -> F) -> ChatStream
where
    F: Future<Output = crate::Result<()>> + Send + 'static,
{
    let (tx, rx) = channel(STREAM_BUFFER);
    let fut = work(tx.clone());
    tokio::spawn(async move {
        if let Err(e) = fut.await {
            let _ = tx.send(Err(e)).await;
        }
    });
    rx
}

/// Index of the first message of the current turn: the one after the last user message.
fn turn_start(messages: &[ChatMessage]) -> usize {
    messages.iter().rposition(|m| m.role == Role::User).map_or(0, |i| i + 1)
}

/// Any server with the OpenAI chat completions API: OpenAI, Azure OpenAI (v1 API), Ollama, vLLM,
/// LiteLLM and many more. Only the base URL, the model and the key differ.
///
/// ```ignore
/// OpenAiCompatible::new("https://api.openai.com/v1", "gpt-4.1-mini").api_key(config.secret("assistant.api_key")?)
/// OpenAiCompatible::new("http://localhost:11434/v1", "qwen3:8b")                       // Ollama
/// OpenAiCompatible::new("https://NAME.openai.azure.com/openai/v1", "DEPLOYMENT").api_key(key).azure()
/// ```
pub struct OpenAiCompatible {
    base: String,
    model: String,
    key: Option<Secret<String>>,
    header: KeyHeader,
    legacy_max_tokens: bool,
    http: reqwest::Client,
}

impl OpenAiCompatible {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        OpenAiCompatible {
            base: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            key: None,
            header: KeyHeader::Bearer,
            legacy_max_tokens: false,
            http: client(),
        }
    }

    /// The API key, sent as a bearer token (or `api-key` with [`OpenAiCompatible::azure`]).
    pub fn api_key(mut self, key: Secret<String>) -> Self {
        self.key = Some(key);
        self
    }

    /// Sends the key in Azure's `api-key` header.
    pub fn azure(mut self) -> Self {
        self.header = KeyHeader::ApiKey;
        self
    }

    /// Sends `max_tokens` instead of `max_completion_tokens`, for older servers.
    pub fn legacy_max_tokens(mut self) -> Self {
        self.legacy_max_tokens = true;
        self
    }

    /// The request body for a round.
    pub fn body(&self, request: &ChatRequest) -> Json {
        let mut messages = vec![json!({ "role": "system", "content": request.system })];
        for m in &request.messages {
            messages.push(match m.role {
                Role::User => json!({ "role": "user", "content": m.text }),
                Role::Tool => json!({ "role": "tool", "tool_call_id": m.tool_call_id, "content": m.text }),
                Role::Assistant if m.tool_calls.is_empty() => json!({ "role": "assistant", "content": m.text }),
                Role::Assistant => json!({
                    "role": "assistant",
                    "content": if m.text.is_empty() { Json::Null } else { json!(m.text) },
                    "tool_calls": m.tool_calls.iter().map(|c| json!({
                        "id": c.id,
                        "type": "function",
                        "function": { "name": c.name, "arguments": c.arguments.to_string() },
                    })).collect::<Vec<_>>(),
                }),
            });
        }
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "stream_options": { "include_usage": true },
            "messages": messages,
        });
        let field = if self.legacy_max_tokens { "max_tokens" } else { "max_completion_tokens" };
        body[field] = json!(request.max_tokens);
        if !request.tools.is_empty() {
            body["tools"] = request
                .tools
                .iter()
                .map(|t| json!({ "type": "function", "function": { "name": t.name, "description": t.description, "parameters": t.schema } }))
                .collect();
        }
        body
    }
}

/// Tool calls being streamed, by index: id, name and the arguments so far.
type Partial = Vec<(String, String, String)>;

/// Reads one chunk of an OpenAI compatible stream into events.
fn openai_chunk(data: &Json, partial: &mut Partial, out: &mut Vec<ChatEvent>) {
    if let Some(u) = data.get("usage").filter(|u| !u.is_null()) {
        let n = |k: &str| u.get(k).and_then(Json::as_u64).unwrap_or(0) as u32;
        out.push(ChatEvent::Usage { input: n("prompt_tokens"), output: n("completion_tokens") });
    }
    let Some(delta) = data.pointer("/choices/0/delta") else { return };
    if let Some(text) = delta.get("content").and_then(Json::as_str).filter(|t| !t.is_empty()) {
        out.push(ChatEvent::Text(text.to_string()));
    }
    for call in delta.get("tool_calls").and_then(Json::as_array).into_iter().flatten() {
        let i = call.get("index").and_then(Json::as_u64).unwrap_or(0).min(63) as usize;
        while partial.len() <= i {
            partial.push(Default::default());
        }
        let p = &mut partial[i];
        if let Some(id) = call.get("id").and_then(Json::as_str) {
            p.0 = id.to_string();
        }
        if let Some(name) = call.pointer("/function/name").and_then(Json::as_str) {
            p.1.push_str(name);
        }
        if let Some(args) = call.pointer("/function/arguments").and_then(Json::as_str) {
            p.2.push_str(args);
        }
    }
}

/// Tool calls that finished streaming. Unreadable arguments become an empty object; the tool
/// then refuses them.
fn finish_calls(partial: Partial) -> Vec<ChatEvent> {
    partial
        .into_iter()
        .filter(|(_, name, _)| !name.is_empty())
        .enumerate()
        .map(|(i, (id, name, args))| {
            let arguments = if args.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&args).unwrap_or_else(|_| json!({}))
            };
            let id = if id.is_empty() { format!("call_{i}") } else { id };
            ChatEvent::ToolCall(ToolCall { id, name, arguments })
        })
        .collect()
}

impl LlmProvider for OpenAiCompatible {
    fn chat(&self, request: ChatRequest) -> BoxFuture<crate::Result<ChatStream>> {
        let req = authorize(self.http.post(format!("{}/chat/completions", self.base)), self.key.as_ref(), self.header)
            .json(&self.body(&request));
        Box::pin(async move {
            let res = send(req, "chat model").await?;
            Ok(spawn_stream(|tx| async move {
                let mut sse = Sse::new(res);
                let mut partial: Partial = Vec::new();
                let mut events = Vec::new();
                while let Some((_, data)) = sse.next().await? {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(chunk) = serde_json::from_str::<Json>(&data) {
                        openai_chunk(&chunk, &mut partial, &mut events);
                    }
                    if !forward(&tx, &mut events).await {
                        return Ok(());
                    }
                }
                events.extend(finish_calls(partial));
                forward(&tx, &mut events).await;
                Ok(())
            }))
        })
    }
}

/// The Anthropic Messages API.
///
/// ```ignore
/// Anthropic::new("claude-opus-5-5").api_key(config.secret("assistant.api_key")?)
/// ```
pub struct Anthropic {
    base: String,
    model: String,
    key: Option<Secret<String>>,
    http: reqwest::Client,
}

impl Anthropic {
    pub fn new(model: impl Into<String>) -> Self {
        Anthropic { base: "https://api.anthropic.com".into(), model: model.into(), key: None, http: client() }
    }

    pub fn api_key(mut self, key: Secret<String>) -> Self {
        self.key = Some(key);
        self
    }

    /// Another address with the same API, such as a gateway.
    pub fn base_url(mut self, base: impl Into<String>) -> Self {
        self.base = base.into().trim_end_matches('/').to_string();
        self
    }

    /// The request body for a round. Results of consecutive tool calls go back in one user
    /// message; content the model asked to keep (reasoning blocks) goes back unchanged within the
    /// current turn only.
    pub fn body(&self, request: &ChatRequest) -> Json {
        let current = turn_start(&request.messages);
        let mut messages: Vec<Json> = Vec::new();
        for (i, m) in request.messages.iter().enumerate() {
            match m.role {
                Role::User => messages.push(json!({ "role": "user", "content": m.text })),
                Role::Tool => {
                    let block = json!({ "type": "tool_result", "tool_use_id": m.tool_call_id, "content": m.text });
                    let joined = messages.last_mut().filter(|l| l["role"] == "user" && l["content"].is_array());
                    match joined.and_then(|l| l["content"].as_array_mut()) {
                        Some(blocks) if blocks.iter().all(|b| b["type"] == "tool_result") => blocks.push(block),
                        _ => messages.push(json!({ "role": "user", "content": [block] })),
                    }
                }
                Role::Assistant => {
                    let content = match &m.raw {
                        Some(raw @ Json::Array(_)) if i >= current => raw.clone(),
                        _ => {
                            let mut blocks = Vec::new();
                            if !m.text.trim().is_empty() {
                                blocks.push(json!({ "type": "text", "text": m.text }));
                            }
                            for c in &m.tool_calls {
                                blocks.push(
                                    json!({ "type": "tool_use", "id": c.id, "name": c.name, "input": c.arguments }),
                                );
                            }
                            Json::Array(blocks)
                        }
                    };
                    if content.as_array().is_some_and(|b| !b.is_empty()) {
                        messages.push(json!({ "role": "assistant", "content": content }));
                    }
                }
            }
        }
        let mut body = json!({
            "model": self.model,
            "max_tokens": request.max_tokens,
            "stream": true,
            "system": request.system,
            "messages": messages,
        });
        if !request.tools.is_empty() {
            body["tools"] = request
                .tools
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.schema }))
                .collect();
        }
        body
    }
}

/// State of an Anthropic stream: the content blocks so far, tool inputs being streamed and usage.
#[derive(Default)]
struct Blocks {
    blocks: Vec<Json>,
    inputs: Vec<String>,
    input_tokens: u32,
    output_tokens: u32,
    keep: bool,
}

impl Blocks {
    /// Reads one event into `out`. Returns `false` at the end of the message.
    fn event(&mut self, name: &str, data: &Json, out: &mut Vec<ChatEvent>) -> crate::Result<bool> {
        let kind = data.get("type").and_then(Json::as_str).unwrap_or(name);
        match kind {
            "message_start" => {
                self.input_tokens =
                    data.pointer("/message/usage/input_tokens").and_then(Json::as_u64).unwrap_or(0) as u32;
            }
            "content_block_start" => {
                let block = data.get("content_block").cloned().unwrap_or(Json::Null);
                if matches!(block["type"].as_str(), Some("thinking" | "redacted_thinking")) {
                    self.keep = true;
                }
                self.blocks.push(block);
                self.inputs.push(String::new());
            }
            "content_block_delta" => {
                let i = data.get("index").and_then(Json::as_u64).unwrap_or(0) as usize;
                let (Some(block), Some(input)) = (self.blocks.get_mut(i), self.inputs.get_mut(i)) else {
                    return Ok(true);
                };
                let delta = &data["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        let text = delta["text"].as_str().unwrap_or("");
                        append(block, "text", text);
                        if !text.is_empty() {
                            out.push(ChatEvent::Text(text.to_string()));
                        }
                    }
                    Some("input_json_delta") => input.push_str(delta["partial_json"].as_str().unwrap_or("")),
                    Some("thinking_delta") => append(block, "thinking", delta["thinking"].as_str().unwrap_or("")),
                    Some("signature_delta") => block["signature"] = delta["signature"].clone(),
                    _ => {}
                }
            }
            "content_block_stop" => {
                let i = data.get("index").and_then(Json::as_u64).unwrap_or(0) as usize;
                if let (Some(block), Some(input)) = (self.blocks.get_mut(i), self.inputs.get(i)) {
                    if block["type"] == "tool_use" {
                        let arguments = if input.trim().is_empty() {
                            json!({})
                        } else {
                            serde_json::from_str(input).unwrap_or_else(|_| json!({}))
                        };
                        block["input"] = arguments.clone();
                        out.push(ChatEvent::ToolCall(ToolCall {
                            id: block["id"].as_str().unwrap_or_default().to_string(),
                            name: block["name"].as_str().unwrap_or_default().to_string(),
                            arguments,
                        }));
                    }
                }
            }
            "message_delta" => {
                self.output_tokens = data.pointer("/usage/output_tokens").and_then(Json::as_u64).unwrap_or(0) as u32;
            }
            "message_stop" => {
                out.push(ChatEvent::Usage { input: self.input_tokens, output: self.output_tokens });
                if self.keep {
                    out.push(ChatEvent::Raw(Json::Array(std::mem::take(&mut self.blocks))));
                }
                return Ok(false);
            }
            "error" => {
                let message = data.pointer("/error/message").and_then(Json::as_str).unwrap_or("unknown");
                return Err(Error::Internal(format!("chat model error: {}", super::clip(message, ERROR_BODY))));
            }
            _ => {}
        }
        Ok(true)
    }
}

fn append(block: &mut Json, key: &str, text: &str) {
    let mut s = block.get(key).and_then(Json::as_str).unwrap_or("").to_string();
    s.push_str(text);
    if let Some(o) = block.as_object_mut() {
        o.insert(key.to_string(), Json::String(s));
    }
}

impl LlmProvider for Anthropic {
    fn chat(&self, request: ChatRequest) -> BoxFuture<crate::Result<ChatStream>> {
        let mut req = self
            .http
            .post(format!("{}/v1/messages", self.base))
            .header("anthropic-version", "2026-10-21")
            .json(&self.body(&request));
        if let Some(k) = &self.key {
            req = req.header("x-api-key", k.expose());
        }
        Box::pin(async move {
            let res = send(req, "chat model").await?;
            Ok(spawn_stream(|tx| async move {
                let mut sse = Sse::new(res);
                let mut state = Blocks::default();
                let mut events = Vec::new();
                while let Some((name, data)) = sse.next().await? {
                    let Ok(event) = serde_json::from_str::<Json>(&data) else { continue };
                    let more = state.event(&name, &event, &mut events)?;
                    if !forward(&tx, &mut events).await || !more {
                        return Ok(());
                    }
                }
                Ok(())
            }))
        })
    }
}

/// Speech to text through an OpenAI compatible `/audio/transcriptions` endpoint (OpenAI, Azure,
/// a local Whisper server, LiteLLM).
pub struct OpenAiTranscription {
    base: String,
    model: String,
    key: Option<Secret<String>>,
    header: KeyHeader,
    http: reqwest::Client,
}

impl OpenAiTranscription {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        OpenAiTranscription {
            base: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            key: None,
            header: KeyHeader::Bearer,
            http: client(),
        }
    }

    pub fn api_key(mut self, key: Secret<String>) -> Self {
        self.key = Some(key);
        self
    }

    pub fn azure(mut self) -> Self {
        self.header = KeyHeader::ApiKey;
        self
    }
}

/// A `multipart/form-data` body with text fields and one file.
fn multipart(fields: &[(&str, &str)], file: &Audio) -> (String, Vec<u8>) {
    let mut bytes = [0u8; 16];
    let _ = getrandom::getrandom(&mut bytes);
    let boundary: String =
        std::iter::once("zeyn".to_string()).chain(bytes.iter().map(|b| format!("{b:02x}"))).collect();
    let mut body = Vec::with_capacity(file.bytes.len() + 512);
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    let ext = match file.mime.split(';').next().unwrap_or("") {
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/m4a" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => "webm",
    };
    let mime: String = file.mime.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(100).collect();
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"speech.{ext}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
    body.extend_from_slice(&file.bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

impl SpeechToText for OpenAiTranscription {
    fn transcribe(&self, audio: Audio, locale: String) -> BoxFuture<crate::Result<String>> {
        let language: String = locale.chars().take_while(|c| c.is_ascii_alphabetic()).take(3).collect();
        let (content_type, body) =
            multipart(&[("model", &self.model), ("language", &language), ("response_format", "json")], &audio);
        let req =
            authorize(self.http.post(format!("{}/audio/transcriptions", self.base)), self.key.as_ref(), self.header)
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(body);
        Box::pin(async move {
            let res = send(req, "speech to text").await?;
            let value: Json =
                res.json().await.map_err(|_| Error::Internal("speech to text: unreadable answer".into()))?;
            Ok(value.get("text").and_then(Json::as_str).unwrap_or_default().trim().to_string())
        })
    }
}

/// Text to speech through an OpenAI compatible `/audio/speech` endpoint. The audio is passed on
/// as it arrives.
pub struct OpenAiSpeech {
    base: String,
    model: String,
    voice: String,
    key: Option<Secret<String>>,
    header: KeyHeader,
    http: reqwest::Client,
}

impl OpenAiSpeech {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, voice: impl Into<String>) -> Self {
        OpenAiSpeech {
            base: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            voice: voice.into(),
            key: None,
            header: KeyHeader::Bearer,
            http: client(),
        }
    }

    pub fn api_key(mut self, key: Secret<String>) -> Self {
        self.key = Some(key);
        self
    }

    pub fn azure(mut self) -> Self {
        self.header = KeyHeader::ApiKey;
        self
    }
}

impl TextToSpeech for OpenAiSpeech {
    fn speak(&self, text: String, _locale: String) -> BoxFuture<crate::Result<SpeechStream>> {
        let mut body = Map::new();
        body.insert("model".into(), json!(self.model));
        body.insert("voice".into(), json!(self.voice));
        body.insert("input".into(), json!(text));
        body.insert("response_format".into(), json!("mp3"));
        let req = authorize(self.http.post(format!("{}/audio/speech", self.base)), self.key.as_ref(), self.header)
            .json(&body);
        Box::pin(async move {
            let mut res = send(req, "text to speech").await?;
            let (tx, rx) = channel::<crate::Result<Bytes>>(STREAM_BUFFER);
            tokio::spawn(async move {
                loop {
                    let next = match res.chunk().await {
                        Ok(Some(c)) => Ok(c),
                        Ok(None) => return,
                        Err(e) => Err(Error::Internal(format!("text to speech stream broke: {}", e.without_url()))),
                    };
                    let stop = next.is_err();
                    if tx.send(next).await.is_err() || stop {
                        return;
                    }
                }
            });
            Ok(SpeechStream { mime: "audio/mpeg".into(), chunks: rx })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_tool_calls_are_put_together_from_pieces() {
        let mut partial = Vec::new();
        let mut out = Vec::new();
        for chunk in [
            json!({ "choices": [{ "delta": { "content": "Hi" } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "id": "call_1", "function": { "name": "navigate", "arguments": "{\"scr" } }] } }] }),
            json!({ "choices": [{ "delta": { "tool_calls": [{ "index": 0, "function": { "arguments": "een\":\"today\"}" } }] } }] }),
            json!({ "choices": [], "usage": { "prompt_tokens": 10, "completion_tokens": 3 } }),
        ] {
            openai_chunk(&chunk, &mut partial, &mut out);
        }
        out.extend(finish_calls(partial));
        assert_eq!(out[0], ChatEvent::Text("Hi".into()));
        assert_eq!(out[1], ChatEvent::Usage { input: 10, output: 3 });
        assert_eq!(
            out[2],
            ChatEvent::ToolCall(ToolCall {
                id: "call_1".into(),
                name: "navigate".into(),
                arguments: json!({ "screen": "today" })
            })
        );
    }

    #[test]
    fn anthropic_events_give_text_tool_calls_usage_and_reasoning_to_keep() {
        let mut state = Blocks::default();
        let mut out = Vec::new();
        for e in [
            json!({ "type": "message_start", "message": { "usage": { "input_tokens": 12 } } }),
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "" } }),
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "signature_delta", "signature": "sig" } }),
            json!({ "type": "content_block_stop", "index": 0 }),
            json!({ "type": "content_block_start", "index": 1, "content_block": { "type": "text", "text": "" } }),
            json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "text_delta", "text": "Opening" } }),
            json!({ "type": "content_block_start", "index": 2, "content_block": { "type": "tool_use", "id": "toolu_1", "name": "navigate", "input": {} } }),
            json!({ "type": "content_block_delta", "index": 2, "delta": { "type": "input_json_delta", "partial_json": "{\"screen\": \"today\"}" } }),
            json!({ "type": "content_block_stop", "index": 2 }),
            json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 7 } }),
        ] {
            assert!(state.event("", &e, &mut out).unwrap());
        }
        assert!(!state.event("", &json!({ "type": "message_stop" }), &mut out).unwrap());
        assert_eq!(out[0], ChatEvent::Text("Opening".into()));
        assert_eq!(
            out[1],
            ChatEvent::ToolCall(ToolCall {
                id: "toolu_1".into(),
                name: "navigate".into(),
                arguments: json!({ "screen": "today" })
            })
        );
        assert_eq!(out[2], ChatEvent::Usage { input: 12, output: 7 });
        let ChatEvent::Raw(Json::Array(blocks)) = &out[3] else { panic!("{out:?}") };
        assert_eq!(blocks[0]["signature"], "sig");
        assert_eq!(blocks[2]["input"], json!({ "screen": "today" }));
    }

    #[test]
    fn anthropic_groups_tool_results_and_keeps_reasoning_only_in_the_current_turn() {
        let a = Anthropic::new("m");
        let mut old = ChatMessage::assistant("earlier");
        old.raw = Some(
            json!([{ "type": "thinking", "thinking": "", "signature": "old" }, { "type": "text", "text": "earlier" }]),
        );
        let mut call = ChatMessage::assistant("");
        call.tool_calls = vec![
            ToolCall { id: "a".into(), name: "x".into(), arguments: json!({}) },
            ToolCall { id: "b".into(), name: "y".into(), arguments: json!({}) },
        ];
        call.raw = Some(
            json!([{ "type": "thinking", "thinking": "", "signature": "new" }, { "type": "tool_use", "id": "a", "name": "x", "input": {} }, { "type": "tool_use", "id": "b", "name": "y", "input": {} }]),
        );
        let messages = vec![
            ChatMessage::user("one"),
            old,
            ChatMessage::user("two"),
            call,
            ChatMessage::tool("a", "r1"),
            ChatMessage::tool("b", "r2"),
        ];
        let body = a.body(&ChatRequest { system: "s".into(), messages, tools: Vec::new(), max_tokens: 10 });
        let m = body["messages"].as_array().unwrap();
        assert_eq!(m.len(), 5);
        assert_eq!(m[1]["content"], json!([{ "type": "text", "text": "earlier" }]));
        assert_eq!(m[3]["content"][0]["signature"], "new");
        assert_eq!(m[4]["content"].as_array().unwrap().len(), 2);
        assert_eq!(body["system"], "s");
    }
}
