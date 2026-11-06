# 管理视图

面向后台的组件：记录中改了什么、只读的代码和数据、条码、通知、看板和日历。每个组件都在服务器上渲染，只有用到它的页面才加载它的样式表和脚本。

## DiffView

`DiffView` 显示一条记录两个值之间改变的字段：旧值加删除线，新值标为新增。可以用 `DiffView::changes(entry.changes)` 从审计记录构建，也可以用 `DiffView::of(&before, &after)` 从模型的两个值构建，后者的比较方式与审计跟踪相同（不含行版本）。

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` 让字段显示用户语言的标签，让选项字段显示选项标签。并排布局是一个表格，一列放旧值，一列放新值；`DiffLayout::Inline` 把新值放在旧值后面，适合窗口这样的窄处。最多显示 `MAX_DIFF_FIELDS`（200）个字段，其余的用一条说明提示。

掩码字段保持掩码。值是审计跟踪保存的掩码文本，模型用 `#[mask]` 标记的字段在显示前会再次掩码，因此即使是手工构建的变更列表也不会把原始值放进页面。该组件不需要脚本；`diff.css`（1.9 KB）只随它加载。

## CodeView 和 JsonView

`CodeView` 以只读方式显示源文本：在服务器上高亮，带行号、折叠和复制按钮。`JsonView` 以同样的方式缩进显示 JSON 值。

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

高亮器（`zeyn::ui::highlight`）是为 Rust、JSON、TOML、SQL、JavaScript 和 shell 手写的；`CodeLang::Plain` 和其他名称显示转义后的文本。它把关键字、字符串、数字、注释、类型和键包在 `z-hl-*` span 中，颜色来自主题令牌，所以两种主题都能用，浏览器也无需做任何工作。跨越多行的括号块（`{}` 和 `[]`）用原生 `<details>` 元素折叠，不需要脚本：第一行就是按钮（Enter 或 Space），折叠后的块以省略号结尾。

`JsonView::of(value, &ctx)` 按端点发送到浏览器的方式序列化任意值，因此掩码字段保持掩码。最多显示 `MAX_CODE_LINES`（10,000）行和 `MAX_CODE_BYTES`（1 MB）；更长的文本会在行尾截断，并附一条说明显示了多少行。复制按钮（`share.js`，也服务于 `Share`）复制显示的行并用提示确认；在脚本运行之前它是隐藏的。`code.css`（2.7 KB）只随该组件加载。

| 按键 | 作用 |
|---|---|
| Tab | 移到文本（一个停靠点，可用方向键滚动），然后依次经过折叠按钮和复制按钮 |
| Enter, Space | 在折叠按钮上：折叠或展开该块 |

## Barcode 和 QrCode

`Barcode::code128(value)` 和 `QrCode::new(value)` 在服务器上把符号绘制成 SVG。两个编码器都按照公开标准（Code 128 和 QR Code）手写，因此不增加任何 crate，也不需要脚本。

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode` 编码 1 到 80 个可打印 ASCII 字符（`MAX_BARCODE_CHARS`），把连续数字两个一组打包（C 码集），并加上校验符号和两侧各 10 个模块的静区。`QrCode` 把文本按 UTF-8 字节编码，在所选级别（`QrLevel::Low`、默认的 `Medium`、`Quartile`、`High`）下选择能容纳它的最小版本（1 到 40），使用 Reed Solomon 纠错、罚分最低的掩码和 4 个模块的静区；最多 `MAX_QR_BYTES`（2,953）字节。无法编码的值会显示一条简短说明，而不是损坏的符号。

每个符号都是一个 figure，屏幕阅读器把它当作一张图片读出，名称是下方的文字：即值本身，或设置了 `.label(text)` 时的标签。在两种主题下条码都保持白底黑条，因为扫描器需要浅底深条。`barcode.css`（1 KB）只随这些组件加载；`.height(px)` 和 `.size(px)` 设置绘制尺寸，容器较窄时符号会缩小。

## Inbox 和 NotificationCenter

`Inbox` 按用户时区的日期分组（今天、昨天，然后是日期）列出用户的通知，最新的在前，并标出未读的。行来自与 DataGrid 相同类型的动作（输入 `DataQuery`，输出 `Page`）；收件箱按时间字段排序，最多请求 `.limit(n)` 行（默认 50，最多 `MAX_INBOX`，即 200）。`NotificationCenter` 是顶栏的铃铛：一个带未读数、指向收件箱屏幕的链接。

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

使用 `.mark_read(action)` 和 `.archive(action)` 后，每行会有一个复选框，收件箱会有一个带“全选”的工具栏。按钮把选中的键作为 `InboxSelection` 通过 `POST /_zeyn/data` 发送，该端点检查 CSRF，计入写入限制，并检查屏幕的访问权限和动作的权限；最多接受 200 个键，每个最多 200 个字符。这些键来自浏览器，所以动作只能修改 `ctx.identity()` 用户的通知。用户无权执行的按钮及复选框都不会显示。

批量操作之后，收件箱会触发它的主题（`"notifications"`，可用 `.topic(..)` 更改）：收件箱和所有 `.listens` 该主题的节点就地重新渲染，铃铛也会重新加载计数。铃铛用一个接收 `UnreadCount { limit }` 并在上限（100）处停止的动作计数，只需一次有上限的查询；超过 99 时显示 `99+`，链接的名称会为屏幕阅读器读出计数。它在页面显示后用一个请求加载计数，并在主题被收件箱、`ui.refresh("notifications")`，或在启用 `.live()` 和 `realtime` 特性时被任意动作中的 `hub.notify(&ctx, "notifications")` 触发时重新加载（主题需用 `Realtime::topic` 声明）。`inbox.js`（6.1 KB，gzip 后 2.3 KB）和 `inbox.css`（3.1 KB）只随这些组件加载。
