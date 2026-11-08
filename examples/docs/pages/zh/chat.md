# 聊天

用于对话的组件：一次对话的消息、用户输入消息的框，以及有人正在输入的提示。每个组件都在服务器上渲染，只在使用它的页面上加载自己的样式表和脚本。

## MessageThread

`MessageThread` 以聊天气泡的形式显示一次对话的消息：用户自己的消息在结束一侧，其他人的消息在开始一侧（在从右到左书写的语言中左右镜像），同一发送者在五分钟内连续发送的消息归在一个名字下，按用户所在时区为每一天加一个标题，显示每条消息的时间，并在自己的消息上显示投递状态。

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

历史记录 action 接收一个 `ThreadQuery`：对话（服务器从组件中取得，浏览器从不发送它）和一个按从新到旧排序的 `DataQuery`。请在 action 中检查用户是否参与该对话。`.author` 是发送者的用户 id，会与已登录的用户比较，以区分自己的消息和他人的消息；`.name` 是显示的名字。模型需要一个 `#[key]`，这样加载了两次的消息只显示一次。

前 50 条消息（`THREAD_PAGE`）随屏幕一起渲染。向上滚动会通过 `POST /_zeyn/grid` 加载下一页更早的消息，每次都会检查屏幕的访问权限和 action 的权限，并让读者正在看的消息保持在原位。页面中最多保留 200 条消息：继续滚动会丢弃远端，读者返回时再重新加载，因此有数千条消息的对话与短对话占用相同的内存。在 5,000 条消息中滚动时，页面中约有 2,000 个元素，并能以显示器的帧率运行。

`Delivery` 有 `Sending`、`Sent`、`Delivered`、`Read`（已读回执）和 `Failed` 几种状态，每种都用一个图标和一段供屏幕阅读器读取的文字表示。`.attachments(field, rules)` 显示消息的文件（一个 `Vec<File>` 字段和一个用 `Feature::files` 注册的存储的规则）：图片放在固定大小的框中，在占位图上延迟加载；其他文件显示为下载链接，并经过存储的读取权限。

当线程的某个 topic 被点名时就会获取新消息，除非 `.topic(..)` 另有指定，否则该 topic 为 `"chat"`：在 `Composer` 发送之后，在某个 action 以 `ui.refresh("chat")` 结束之后，或者在有 `.live()` 和 `realtime` feature 时，任何地方运行 `hub.notify(&ctx, "chat")` 时。他人的消息会在 polite 实时区域中播报。

### AI 回复

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

使用 `.reply(action)` 时，通过同一对话的 `Composer` 发送的消息会启动一次 AI 回复：浏览器把文本发送到 `POST /_zeyn/stream`（CSRF、写入限制、屏幕的访问权限、action 的权限和拦截器），回复以内嵌助手的格式作为 JSON 行流式返回。各个片段经过一个有界通道（`REPLY_BUFFER`）；服务器在 `MAX_REPLY_CHARS`（32,000）个字符或 `REPLY_SECONDS`（120）秒后停止，停止按钮可以提前结束回复。读者离开后 `ReplySender::send` 返回 `false`，这就是生成方应当停止的信号。

回复显示为一个只由文本节点构建的小型 Markdown 子集，因此模型写出的任何内容都不会被当作 HTML 解析。之前的消息从不重新渲染，只渲染正在写的段落：一个由 2,000 个片段组成的回复，到结尾时每帧的开销与开始时相同，上方的内容也不会移动。实时区域在回复完成后朗读一次，从不逐个片段朗读。用 `.ai(field)` 标记的消息在从历史记录加载时同样作为安全的 Markdown 显示（需要 `markdown` feature；没有它时作为纯文本）。

回复的键（`Reply::channel(key)`）就是保存后的消息将拥有的键，因此历史记录会在原位显示保存的回答，而不是第二份副本。

## Composer

`Composer` 是用户输入消息的框：一个随文本增高、直到最大高度的文本区域，一个发送按钮，以及按需提供的附件、提及和保留的草稿。

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

发送 action 接收一个 `ChatSend`：服务器从组件中取得的对话、去掉首尾空白的文本、附加的文件以及被提及者的 id。发送通过带 CSRF 令牌的 `POST /_zeyn/data` 进行，计入写入限制，并需要 action 的权限；用户无权发送的 composer 不会出现在页面中。服务器会拒绝空消息、长于 `.max_chars(n)`（未设置时为 4,000）的消息、超过存储规则允许数量的文件、格式错误的文件 id 以及超过 20 个的提及。其余内容也应视为不可信：请检查用户是否参与该对话，以及是否可以提及这些人。

默认的 `SendKey::Enter` 用 Enter 发送，用 Shift+Enter 换行；`.send_key(SendKey::CtrlEnter)` 用 Ctrl+Enter（Mac 上为 Cmd+Enter）发送，Enter 留给换行。消息为空或过长时发送按钮处于禁用状态，接近上限时会显示一个计数器。发送后 composer 会点名自己的 topic（除非 `.topic(..)` 另有指定，否则为 `"chat"`），因此同一对话的 `MessageThread` 会立即显示该消息，并在设置了 `.reply(..)` 时流式显示 AI 回复。

`.attachments(rules)` 添加一个用 `Feature::files` 注册的存储的上传字段：文件带进度条逐个上传到 `POST /_zeyn/files/{store}`，经过存储的写入权限和类型检查，它们的 id 随消息一起发送。

`.mentions(action)` 在 `@` 之后用一个 lookup action 搜索人员（输入 `LookupQuery`，输出一页 `LookupItem`）。搜索在最后一次按键 200 ms 后开始，新的搜索会取消旧的搜索，服务器最多请求 `MAX_MENTIONS`（8）个人，返回的也不超过这个数量。该列表是一个由文本区域驱动的 listbox：方向键移动，Enter 或 Tab 选择，Escape 关闭。

`.remember(key)` 把每个对话中未发送的文本保存在客户端偏好存储中，使用一个用 `App::client_prefs` 注册的 `Draft` 键，直到它被发送。一份草稿最多 `MAX_DRAFT_CHARS`（4,000）个字符，最多保留 `MAX_DRAFTS`（20）个对话，最旧的先被移除。对话 id 只以哈希形式保存。使用 `Purpose::Preference` 时，在用户同意更多之前，草稿只在会话期间保留。

## TypingIndicator

`TypingIndicator` 在 polite 实时区域中显示对话中还有谁正在输入：“Ada 正在输入”“Ada 和 Grace 正在输入”“3 人正在输入”。

```rust
let realtime = Realtime::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

它通过实时通道（`realtime` feature，见 `realtime.md`）在一个用 `Realtime::topic` 声明的 topic 上工作。当用户在同一对话的 `Composer` 中输入时，浏览器通过页面的实时 socket 最多每 3 秒发送一个小信号；服务器按用户丢弃更频繁的信号，并把信号传给租户内该 topic 的其他订阅者，从不回传给同一用户。信号包含对话 id 和一个只有本服务器能生成的用户键，从不包含用户 id 或任何文本。名字在最后一个信号后显示 6 秒；没有通过 `.person(user, name)` 提供的人显示为“某人”。

没有 `realtime` feature，或应用没有实时通道时，指示器不渲染任何内容，它的脚本不会加载，也不会发送任何东西。信号会到达租户内该 topic 的每个订阅者，因此对话 id 对他们可见：请使用不透明的 id，或为每类受众使用单独的 topic。
