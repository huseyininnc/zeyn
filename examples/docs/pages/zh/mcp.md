# MCP 与 AI 智能体

Zeyn 从两个方面为智能体而设计：编写 Zeyn 应用的智能体，以及代表用户使用运行中应用的智能体。

## 编写应用的智能体

智能体工作所在的项目自带规则和工具：

- `AGENTS.md` 和 `CLAUDE.md` 列出命令、项目结构、语言规则和框架索引。
- `.mcp.json` 注册 `zeyn mcp`，这是一个基于 stdio 的 MCP 服务器，提供以下工具：

| 工具 | 作用 |
|---|---|
| `zeyn_docs` | 按主题提供框架参考：`rules`、`language`、`protocol`；`search: "关键词"` 对此版本的文档页面排序，`page: "/language/forms"` 以 Markdown 返回一页 |
| `zeyn_describe` | 应用的 feature、界面、路由、访问控制、API、问题和缺失的翻译 |
| `zeyn_check` | 编译并报告问题；`fix: true` 会先重写已更名的 API |
| `zeyn_test` | 运行 `cargo test` 并返回输出的末尾部分 |
| `zeyn_new_feature` | 创建 feature 文件夹并注册它 |
| `zeyn_new_screen` | 为 feature 添加屏幕文件并注册它 |

编译器是主要的把关者。它能够强制执行的每条规则都有一个 `compile_fail` 测试和一条供智能体据以行动的 `#[diagnostic::on_unimplemented]` 消息，例如：

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run` 会打印与 `zeyn_describe` 返回内容相同的描述，然后退出。

## 使用应用的智能体

```rust
App::new(t!("app.name")).mcp(Mcp::read_only())
```

使用 `agents` 后，应用的界面会成为工具：

- 每个表格对应一个搜索工具，
- 每个表单对应一个保存工具，以表单的字段和规则作为其模式（仅限 `Mcp::read_write()`）。

智能体通过两种方式访问它们：

- **在浏览器中**，在浏览器支持时通过 WebMCP（`document.modelContext.registerTool`）。当前界面的工具在其打开时注册，关闭时移除。
- **在服务器上**，通过 `POST /_zeyn/mcp`（JSON-RPC）。它需要 `Authorization` 头，因此仅凭浏览器会话永远无法驱动它。

## 与人相同的规则

- 智能体获得的权限与其所代表用户的权限完全一致。用户无权打开的界面和 action 所对应的工具不会被列出。
- 调用会运行普通调用所具有的 guard、拦截器、校验和速率限制。
- 审计记录会标记由智能体发起的调用，以便审查者区分它们。
- `Mcp::read_only()` 允许智能体搜索但不能保存。只有在希望智能体修改数据的应用中才选择 `read_write()`。
