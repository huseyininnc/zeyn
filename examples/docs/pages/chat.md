# Chat

Components for conversations: the messages of a conversation, the box a user writes in, and the note that someone is typing. Each one is rendered on the server and loads its stylesheet and script only on pages that use it.

## MessageThread

`MessageThread` shows the messages of one conversation as chat bubbles: the user's own messages on the end side, the others on the start side (mirrored in right to left languages), consecutive messages of one sender within five minutes grouped under one name, a heading for every day in the user's time zone, the time of each message and, on own messages, its delivery state.

```rust
#[action(permission = "chat.read")]
pub async fn conversation_messages(q: ThreadQuery, store: Chats, ctx: Ctx) -> Result<Page<Msg>> {
    store.check_member(&q.conversation, &ctx)?;
    Ok(q.query.apply(store.messages(&q.conversation)))
}

MessageThread::of(conversation_messages)
    .conversation(params.id.to_string())
    .author(Msg::author)
    .name(Msg::author_name)
    .text(Msg::body)
    .when(Msg::sent_at)
    .status(Msg::status)
    .attachments(Msg::files, CHAT_FILES)
    .live()
```

The history action takes a `ThreadQuery`: the conversation, which the server takes from the component (the browser never sends it), and a `DataQuery` sorted newest first. Check in the action that the user takes part in the conversation. `.author` is the sender's user id, compared with the signed in user to tell own messages from the others; `.name` is the name shown. The model needs a `#[key]`, so a message that loads twice shows once.

The first 50 messages (`THREAD_PAGE`) are rendered with the screen. Scrolling up loads the next older page through `POST /_zeyn/grid`, with the screen's access and the action's permission checked each time, and keeps the message the reader looks at in its place. At most 200 messages stay in the page: going further drops the far end, which loads again when the reader comes back, so a conversation of thousands of messages costs the same memory as a short one. Scrolling through 5,000 messages runs at the display's frame rate with about 2,000 elements in the page.

`Delivery` has the states `Sending`, `Sent`, `Delivered`, `Read` (the read receipt) and `Failed`, each shown with an icon and a text for screen readers. `.attachments(field, rules)` shows the files of a message (a `Vec<File>` field and the rules of a store registered with `Feature::files`): images in a fixed box that loads lazily over a placeholder, other files as download links that go through the store's read permission.

New messages are fetched when one of the thread's topics is named, `"chat"` unless `.topic(..)` says otherwise: after a `Composer` sends, after an action ends with `ui.refresh("chat")`, or with `.live()` and the `realtime` feature, when `hub.notify(&ctx, "chat")` runs anywhere. Messages of others are announced in a polite live region.

### AI replies

```rust
#[action(permission = "chat.ai")]
pub async fn ask_model(r: ReplyRequest, store: Chats) -> Result<Reply> {
    let id = store.reserve_id();
    let (tx, reply) = Reply::channel(id.to_string());
    tokio::spawn(async move {
        let mut text = String::new();
        for piece in my_model_stream(&r.text).await {
            text.push_str(&piece);
            if !tx.send(piece).await {
                break;
            }
        }
        store.save_ai_answer(id, &r.conversation, text);
    });
    Ok(reply)
}

MessageThread::of(conversation_messages)
    .conversation(id)
    .ai(Msg::from_ai)
    .reply(ask_model)
```

With `.reply(action)`, a message sent with a `Composer` of the same conversation starts an AI reply: the browser posts the text to `POST /_zeyn/stream` (CSRF, the write limit, the screen's access, the action's permission and the interceptors) and the reply streams back as JSON lines in the format of the embedded assistant. The pieces pass through a bounded channel (`REPLY_BUFFER`); the server stops after `MAX_REPLY_CHARS` (32,000) characters or `REPLY_SECONDS` (120) seconds, and a stop button ends the reply early. `ReplySender::send` answers `false` once the reader is gone, which is the generator's signal to stop.

The reply is shown as a small Markdown subset built from text nodes only, so nothing the model writes is ever parsed as HTML. Earlier messages are never rendered again, and only the paragraph being written is: a reply of 2,000 pieces costs the same per frame at its end as at its start, and nothing above it moves. The live region speaks the reply once it is finished, never every piece. Messages marked with `.ai(field)` load from the history as safe Markdown too (with the `markdown` feature; as plain text without it).

The reply's key (`Reply::channel(key)`) is the key the stored message will have, so the history shows the stored answer in its place instead of a second copy.

## Composer

`Composer` is the box a user writes messages in: a text area that grows with its text up to a maximum height, a send button, and on request attachments, mentions and a kept draft.

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, hub: Hub, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    hub.notify(&ctx, "chat");
    Ok(())
}

pub const CHAT_DRAFTS: ClientPref<Draft> =
    ClientPref::draft("chat-draft", Duration::from_secs(7 * 86_400), Purpose::Preference);

Composer::of(send_message)
    .conversation(params.id.to_string())
    .attachments(CHAT_FILES)
    .mentions(search_people)
    .remember(CHAT_DRAFTS)
```

The send action takes a `ChatSend`: the conversation, which the server takes from the component, the trimmed text, the attached files and the ids of the people mentioned. Sending goes through `POST /_zeyn/data` with the CSRF token, counts against the write limit and needs the action's permission; a composer the user may not send with is left out of the page. The server refuses an empty message, one longer than `.max_chars(n)` (4,000 unless set), more files than the store's rules allow, file ids of the wrong shape and more than 20 mentions. Treat the rest as untrusted too: check that the user takes part in the conversation and may name the people mentioned.

`SendKey::Enter`, the default, sends with Enter and starts a new line with Shift+Enter; `.send_key(SendKey::CtrlEnter)` sends with Ctrl+Enter (Cmd+Enter on a Mac) and leaves Enter for new lines. The send button is disabled while the message is empty or too long, and a counter shows near the limit. After a send the composer names its topics (`"chat"` unless `.topic(..)` says otherwise), so a `MessageThread` of the same conversation shows the message at once, and streams its AI reply when it has `.reply(..)`.

`.attachments(rules)` adds the upload field of a store registered with `Feature::files`: files upload one by one to `POST /_zeyn/files/{store}` with a progress bar, under the store's write permission and type checks, and their ids go with the message.

`.mentions(action)` searches people after `@` with a lookup action (`LookupQuery` in, a page of `LookupItem` out). The search starts 200 ms after the last key, a newer search cancels the older one, and the server asks for at most `MAX_MENTIONS` (8) people and sends at most that many back. The list is a listbox driven from the text area: arrows move, Enter or Tab picks, Escape closes.

`.remember(key)` keeps the unsent text of each conversation in the client preference store under a `Draft` key registered with `App::client_prefs`, until it is sent. A draft is at most `MAX_DRAFT_CHARS` (4,000) characters, at most `MAX_DRAFTS` (20) conversations are kept and the oldest goes first. The conversation id is stored only as a hash. With `Purpose::Preference` drafts stay for the session until the user consents to more.

## TypingIndicator

`TypingIndicator` shows who else is typing in a conversation: "Ada is typing", "Ada and Grace are typing", "3 people are typing", in a polite live region.

```rust
let realtime = Realtime::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

It works over the realtime channel (feature `realtime`, see `realtime.md`) on a topic declared with `Realtime::topic`. While the user types in a `Composer` of the same conversation, the browser sends one small signal on the page's live socket at most every 3 seconds; the server drops faster ones per user and passes the signal to the other subscribers of the topic in the tenant, never back to the same user. A signal holds the conversation id and a key of the user that only this server can make, never the user id or any text. A name shows for 6 seconds after the last signal; people not given with `.person(user, name)` show as "someone".

Without the `realtime` feature, or when the app has no realtime channel, the indicator renders nothing, its script never loads and nothing is sent. Signals reach every subscriber of the topic in the tenant, so the conversation id is visible to them: use opaque ids, or a topic per audience.
