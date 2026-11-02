# Embedded assistant

Status: implemented behind the `assistant` Cargo feature of `zeyn`.

A Zeyn app can carry its own AI assistant: a chat on every page that answers with the app's data
and works the app's screens and APIs as the signed in user. Apps built with Zeyn can switch it on
for their own customers, connect the model those customers want, and keep chat history wherever
they need it.

```toml
zeyn = { version = "0.0.2", features = ["assistant"] }
```

Without the feature nothing of it is compiled: no HTTP client, no extra routes, no extra memory.

## 1. Setup

```rust
use zeyn::assistant::{Assistant, OpenAiCompatible};

let model = OpenAiCompatible::new(config.require("assistant.base_url")?, config.require("assistant.model")?)
    .api_key(config.secret("assistant.api_key")?);

App::new("Lifts")
    .assistant(
        Assistant::new(model)
            .permission("lifts.assistant.use")
            .instructions("You help service engineers plan elevator maintenance.")
            .suggest(work_orders, "Which lifts are overdue for service?")
            .suggest_anywhere("What can you do?"),
    )
```

That is all. Every page gets a floating launcher that opens the chat as a side panel. A screen can
also show the chat itself:

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` fits next to other content. `Assistant::launcher(false)` drops the
floating launcher, so the chat appears only where a screen places it.

| Option | Default |
|---|---|
| `permission("...")` | every signed in user |
| `instructions("...")` | none; written by the app's developers, never by users |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | no starter chips |
| `speech_to_text(p)`, `text_to_speech(p)` | the browser's Web Speech API |
| `no_voice()` | microphone and read aloud on |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | the assistant may also save, after confirmation |
| `rate_limit(RateLimit)` | 20 requests a minute per user |
| `token_limit(Some(RateLimit))` | 200,000 tokens an hour per user |
| `limits(Limits { .. })` | see section 6 |

## 2. Models

Any type implementing `LlmProvider` works: one method that starts a streamed round and returns a
bounded channel of `ChatEvent`s (text pieces, tool calls, usage). Built in:

| Provider | Covers |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI (`https://api.openai.com/v1`), Azure OpenAI v1 API (`https://NAME.openai.azure.com/openai/v1`, deployment as model, `.azure()` for the `api-key` header), Ollama (`http://localhost:11434/v1`), vLLM, LiteLLM and other compatible servers. `.legacy_max_tokens()` for servers that do not know `max_completion_tokens` |
| `Anthropic::new(model)` | The Anthropic Messages API (`.base_url(...)` for a gateway). Reasoning blocks are sent back unchanged within a turn |
| `ScriptedProvider` | Tests, demos and offline development: rules instead of a network call, streamed word by word, real tool calls |

Keys come from `Secret` values (`Config::secret`), stay on the server and never appear in error
messages. Provider errors are logged on the server; the user sees a generic message.

Pointing the showcase at a real model takes configuration only:

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

or `NORTHWIND_ASSISTANT__PROVIDER=anthropic` with `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5`, or
an Ollama URL with no key. `NORTHWIND_ASSISTANT__PROVIDER=off` turns it off.

## 3. Speech

By default the microphone uses the browser's speech recognition and "read aloud" the browser's
speech synthesis: no server cost, no audio leaves the browser for the app. For server speech:

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

Both speak the OpenAI compatible `/audio/transcriptions` and `/audio/speech` endpoints, which local
Whisper and speech servers also offer. Recordings are capped (4 MB, 60 seconds in the browser);
speech audio streams through to the browser as it arrives.

## 4. History

`ChatStore` keeps conversations keyed by tenant and user (`ChatKey`), so tenants never share one:

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` is the default (1,000 conversations of 60
messages, least recently used out first, lost on restart). For a database, implement the three
methods over a table with tenant, user, position and the message as JSON. Messages are append
only; the store decides retention. Pending confirmations live in the history too, so the server
holds no other per user state.

## 5. Tools and actions

The model sees the same tools as MCP and WebMCP (see `docs/runtime-protocol.md` section 9), built
for the signed in user each time: grid searches, form saves and API actions the user may use, minus
everything marked `ai = false`, with descriptions from the `///` doc comments and `ai = "..."`
guidance. Two built in tools are added:

- `navigate`: opens a routed screen the user may open, without a page load (`zeyn:navigate`), and
  can filter its grid (`zeyn:filter`). The grid then loads through `POST /_zeyn/grid`, where the
  server applies its usual checks.
- `suggest`: follow up prompts shown as chips.

Every call runs through the normal action path with the context marked as an agent: the screen's
access, the feature's guards, the action's permission, interceptors, rate limits, the time limit,
audit (every assistant call is recorded with `via_agent: true`) and masking.

**Confirmation.** Reads run at once. Anything that writes (form saves, API actions whose input is
not a `DataQuery`) and every action marked `#[action(confirm)]` stops the round and shows a
confirmation card with what will run and its arguments. Only the user's click
(`POST /_zeyn/assistant/confirm`, with CSRF, for a call pending in that user's own history) runs it,
once. A message such as "I confirm" is just text: a new message closes the pending call as not run.

## 6. Security model

| Risk | What the framework does |
|---|---|
| Leaked API keys | Keys are `Secret`s on the server; the browser talks only to the app |
| A model acting beyond the user | Tools are built from the user's permissions; each call is checked again on the server |
| Writes the user did not want | Writes and `confirm` actions need the user's click; the model cannot confirm |
| Masked data reaching the model | Masked fields are always masked for agent calls, even for users who may reveal them |
| Prompt injection through data | Tool results are plain text inside `<tool_result>` delimiters with `<` and `>` escaped; the system prompt says they are data. The system prompt is rebuilt by the server each round from fixed rules, the app's instructions and the screen name, never from user, page or tool data |
| Exfiltration through links | Answers are rendered as a small Markdown subset built with `textContent`: no HTML, no links, no images |
| Runaway cost | Per user request rate limit, per user token limit, capped output tokens, at most 6 rounds and 4 tool calls per round |
| Large inputs | Messages up to 2,000 characters, 30 history messages per round, tool results cut to 6,000 characters, recordings up to 4 MB |
| Cross site requests | Every `POST` needs the CSRF header and counts against the app's write limit |
| Unaccountable actions | Every assistant tool call is audited with `via_agent` |

What it does not do: decide whether a model's answer is right. Tell users so (the panel does) and
keep actions that matter behind `confirm`.

## 7. Embedding for customers

An app that sells the assistant to its own customers usually wants:

- `permission("app.assistant.use")` and a license module check in a guard, so only paying tenants
  see it;
- the model per tenant: implement `LlmProvider` as a small router that picks the customer's
  provider from configuration by `ctx` tenant, or run one model for everyone;
- a `ChatStore` on the app's database, with the customer's retention rules;
- `instructions(...)` that describe the domain (an elevator company: units, sites, service
  intervals), and `ai = "..."` guidance on the actions that need it;
- `token_limit` set to the plan the customer pays for.

## 8. Endpoints

| Endpoint | Does |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | Streams `application/x-ndjson` events |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | Runs or declines the pending call, then streams like chat |
| `POST /_zeyn/assistant/clear` | Deletes the conversation; `204` |
| `POST /_zeyn/assistant/stt` (audio body) | `{ text }`; `404` without server speech to text |
| `POST /_zeyn/assistant/tts` `{ text }` | Streamed audio; `404` without server text to speech |

Events, one JSON object per line: `{t:"text",v}`, `{t:"tool",name,id}`, `{t:"navigate",url,screen,filters}`,
`{t:"suggest",v}`, `{t:"confirm",id,tool,title,question,details}`, `{t:"resolved",id,approved}`,
`{t:"error",message}`, `{t:"done"}`.

`assistant.js` and `assistant.css` load only for users who may use the assistant. The chat log is a
`role="log"` live region (`aria-busy` while an answer streams), Enter sends, Shift+Enter starts a
new line, Escape closes the side panel and returns focus to the launcher, and the confirmation card
puts focus on Cancel first.

## 9. Cost

The assistant holds no per user state in memory except the capped history store and the streams in
flight. Answers stream end to end through channels of 32 items. Measured on the showcase (debug
build, Windows, idle after five page loads): 14.4 MB working set and 3.13 MB private memory with the
assistant off, 14.5 MB and 3.19 MB with it on (about 100 KB more).
