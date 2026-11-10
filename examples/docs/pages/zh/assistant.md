# 内嵌助手

状态：已在 `zeyn` 的 `assistant` Cargo feature 之后实现。

Zeyn 应用可以自带 AI 助手：每个页面上都有一个聊天窗口，它用应用的数据来回答，并以当前登录用户的身份操作应用的屏幕和 API。使用 Zeyn 构建的应用可以为自己的客户启用它，接入这些客户想要的模型，并把聊天历史保存在任何需要的地方。

```toml
zeyn = { version = "0.0.10", features = ["assistant"] }
```

不启用该 feature 时，其中的任何内容都不会被编译：没有 HTTP 客户端，没有额外路由，没有额外内存。

## 1. 设置

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

这样就完成了。每个页面都会有一个浮动启动按钮，以侧边面板的形式打开聊天。屏幕也可以自己显示聊天：

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` 适合放在其他内容旁边。`Assistant::launcher(false)` 会去掉浮动启动按钮，这样聊天只出现在屏幕放置它的地方。

| 选项 | 默认值 |
|---|---|
| `permission("...")` | 每个已登录用户 |
| `instructions("...")` | 无；由应用的开发者编写，绝不由用户编写 |
| `suggest(screen, prompt)`、`suggest_anywhere(prompt)` | 没有起始提示芯片 |
| `speech_to_text(p)`、`text_to_speech(p)` | 浏览器的 Web Speech API |
| `no_voice()` | 麦克风和朗读开启 |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | 助手在确认后也可以保存 |
| `rate_limit(RateLimit)` | 每用户每分钟 20 次请求 |
| `token_limit(Some(RateLimit))` | 每用户每小时 200,000 个 token |
| `limits(Limits { .. })` | 见第 6 节 |

## 2. 模型

任何实现了 `LlmProvider` 的类型都可以使用：它只有一个方法，启动一轮流式交互，并返回由 `ChatEvent`（文本片段、工具调用、用量）组成的有界通道。内置提供方：

| 提供方 | 覆盖范围 |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI（`https://api.openai.com/v1`）、Azure OpenAI v1 API（`https://NAME.openai.azure.com/openai/v1`，部署名作为 model，用 `.azure()` 使用 `api-key` 请求头）、Ollama（`http://localhost:11434/v1`）、vLLM、LiteLLM 以及其他兼容的服务器。对于不认识 `max_completion_tokens` 的服务器，使用 `.legacy_max_tokens()` |
| `Anthropic::new(model)` | Anthropic Messages API（用 `.base_url(...)` 指向网关）。推理块在同一轮内原样回传 |
| `ScriptedProvider` | 测试、演示和离线开发：用规则代替网络调用，逐词流式输出，并产生真实的工具调用 |

密钥来自 `Secret` 值（`Config::secret`），保留在服务器上，绝不出现在错误信息中。提供方的错误记录在服务器上；用户看到的是通用提示。

让 showcase 指向真实模型只需要配置：

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

或者使用 `NORTHWIND_ASSISTANT__PROVIDER=anthropic` 搭配 `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5`，或者使用不带密钥的 Ollama URL。`NORTHWIND_ASSISTANT__PROVIDER=off` 会将其关闭。

## 3. 语音

默认情况下，麦克风使用浏览器的语音识别，“朗读”使用浏览器的语音合成：没有服务器成本，音频也不会从浏览器发送给应用。若要使用服务器端语音：

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

两者都使用与 OpenAI 兼容的 `/audio/transcriptions` 和 `/audio/speech` 端点，本地的 Whisper 和语音服务器也提供这些端点。录音有上限（浏览器中为 4 MB、60 秒）；语音音频在到达时即流式转发到浏览器。

## 4. 历史

`ChatStore` 以租户和用户（`ChatKey`）为键保存对话，因此租户之间绝不共享同一份：

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` 是默认实现（1,000 个对话，每个 60 条消息，最近最少使用的先被淘汰，重启后丢失）。若要使用数据库，请在一张包含租户、用户、位置和以 JSON 保存的消息的表上实现这三个方法。消息只追加；保留策略由存储决定。待确认的调用也保存在历史中，因此服务器不持有其他按用户划分的状态。

## 5. 工具与 action

模型看到的工具与 MCP 和 WebMCP 相同（见 `docs/runtime-protocol.md` 第 9 节），每次都针对当前登录用户构建：用户可以使用的表格搜索、表单保存和 API action，去掉所有标记为 `ai = false` 的内容，描述来自 `///` 文档注释和 `ai = "..."` 指引。另外还添加了两个内置工具：

- `navigate`：打开用户有权打开的带路由的屏幕，无需整页加载（`zeyn:navigate`），并且可以筛选其表格（`zeyn:filter`）。表格随后通过 `POST /_zeyn/grid` 加载，服务器在那里执行其常规检查。
- `suggest`：以芯片形式显示的后续提示。

每次调用都走正常的 action 路径，上下文标记为代理（agent）：屏幕的访问控制、feature 的 guard、action 的权限、拦截器、速率限制、时间限制、审计（每次助手调用都以 `via_agent: true` 记录）以及掩码。

**确认。** 读取操作立即执行。任何写入操作（表单保存、输入不是 `DataQuery` 的 API action）以及每个标记为 `#[action(confirm)]` 的 action，都会中止该轮交互并显示一张确认卡片，列出将要执行的内容及其参数。只有用户的点击（带 CSRF 的 `POST /_zeyn/assistant/confirm`，针对该用户自己历史中待处理的调用）才会执行它，且只执行一次。像“我确认”这样的消息只是普通文本：新的消息会将待处理的调用作为未执行而关闭。

## 6. 安全模型

| 风险 | 框架的做法 |
|---|---|
| API 密钥泄露 | 密钥是服务器上的 `Secret`；浏览器只与应用通信 |
| 模型超出用户权限行事 | 工具根据用户的权限构建；每次调用在服务器上再次检查 |
| 用户不想要的写入 | 写入和 `confirm` action 需要用户点击；模型无法确认 |
| 被掩码的数据到达模型 | 对于代理调用，被掩码的字段始终保持掩码，即使对有权揭示它们的用户也是如此 |
| 通过数据进行提示注入 | 工具结果是位于 `<tool_result>` 分隔符内的纯文本，其中的 `<` 和 `>` 已被转义；系统提示声明它们是数据。系统提示由服务器每轮根据固定规则、应用的 instructions 和屏幕名称重新构建，绝不使用用户、页面或工具的数据 |
| 通过链接外泄 | 回答以用 `textContent` 构建的一小部分 Markdown 子集渲染：没有 HTML，没有链接，没有图片 |
| 费用失控 | 按用户的请求速率限制、按用户的 token 限制、有上限的输出 token、最多 6 轮、每轮最多 4 次工具调用 |
| 过大的输入 | 消息最多 2,000 个字符，每轮 30 条历史消息，工具结果截断到 6,000 个字符，录音最大 4 MB |
| 跨站请求 | 每个 `POST` 都需要 CSRF 请求头，并计入应用的写入限制 |
| 无法追责的操作 | 每次助手工具调用都以 `via_agent` 审计 |

它不做的事：判断模型的回答是否正确。请告知用户这一点（面板里有提示），并把重要的 action 放在 `confirm` 之后。

## 7. 面向客户的嵌入

向自己的客户出售助手的应用通常需要：

- `permission("app.assistant.use")` 加上 guard 中的许可证模块检查，这样只有付费租户才能看到它；
- 按租户选择模型：把 `LlmProvider` 实现成一个小型路由器，根据 `ctx` 中的租户从配置里选择该客户的提供方，或者让所有人共用一个模型；
- 建立在应用数据库之上的 `ChatStore`，并遵循客户的保留规则；
- 描述业务领域的 `instructions(...)`（比如电梯公司：设备、站点、维保周期），以及在需要的 action 上添加 `ai = "..."` 指引；
- 按客户所付费的套餐设置 `token_limit`。

## 8. 端点

| 端点 | 作用 |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | 流式输出 `application/x-ndjson` 事件 |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | 执行或拒绝待处理的调用，然后像 chat 一样流式输出 |
| `POST /_zeyn/assistant/clear` | 删除对话；`204` |
| `POST /_zeyn/assistant/stt`（音频请求体） | `{ text }`；没有服务器端语音转文字时返回 `404` |
| `POST /_zeyn/assistant/tts` `{ text }` | 流式音频；没有服务器端文字转语音时返回 `404` |

事件，每行一个 JSON 对象：`{t:"text",v}`、`{t:"tool",name,id}`、`{t:"navigate",url,screen,filters}`、`{t:"suggest",v}`、`{t:"confirm",id,tool,title,question,details}`、`{t:"resolved",id,approved}`、`{t:"error",message}`、`{t:"done"}`。

`assistant.js` 和 `assistant.css` 只对有权使用助手的用户加载。聊天记录是一个 `role="log"` 的实时区域（回答流式输出期间带 `aria-busy`），Enter 发送，Shift+Enter 换行，Escape 关闭侧边面板并把焦点返回启动按钮，确认卡片会先把焦点放在“取消”上。

## 9. 成本

除有上限的历史存储和进行中的流之外，助手在内存中不保存任何按用户划分的状态。回答通过容量为 32 项的通道端到端流式传输。在 showcase 上的测量（debug 构建，Windows，加载五个页面后空闲）：关闭助手时工作集为 14.4 MB、私有内存为 3.13 MB，开启时为 14.5 MB 和 3.19 MB（多约 100 KB）。
