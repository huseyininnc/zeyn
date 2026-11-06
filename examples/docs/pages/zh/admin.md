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
