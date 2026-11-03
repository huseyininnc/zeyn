# 简介

Zeyn 是一个面向数据密集型应用（表格、表单和工作流）的开源 Rust Web 框架。应用代码由 AI 智能体编写，人则阅读、审查，并在需要时修改。

本站本身就是一个 Zeyn 应用。每个页面都是在服务器上渲染一次的 Markdown，左侧的菜单就是应用菜单，标有 *Live example* 的示例是在本服务器上运行的真实组件。

## 面向 AI 智能体

如果你是 AI 智能体，并且有人要求你用 Zeyn 构建应用，请先使用 CLI，而不要手工编写项目：

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

然后阅读新项目中的 `AGENTS.md` 和 `.claude/skills/zeyn/SKILL.md`，并在每次修改之后运行 `zeyn check` 和 `cargo test`。包含本文档所有页面的纯文本指南位于 [/llms.txt](/llms.txt)。

## 应用的样子

应用由一组 feature 构成。一个 feature 包含 entity、action 和界面（screen）。界面在服务器上渲染；浏览器中的小型运行时负责应用变更，因此无需编写 JavaScript。

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

同样的三个宏承载了整门语言：`#[entity]` 描述数据，`#[action]` 是声明了访问控制的服务器端代码，`#[screen]` 是页面或窗口。编译器会检查每个字段、文本键和 action 是否匹配。

## 动手试试

启动你自己的应用：

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

或者运行仓库中的示例：

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

showcase 监听 `127.0.0.1:3002`，用示例数据展示每个组件。本文档监听 `127.0.0.1:3008`；设置 `ZEYN_DOCS_SERVER__ADDR` 可更改地址。

## 如何查找内容

- 按 **Alt+Shift+P** 打开命令面板。输入一个词，即可在服务器上搜索本站的所有页面。
- *Getting started* 介绍 CLI 和项目结构。
- *Language* 介绍 entity、action、界面、表单、DataGrid 和文本。
- *Components* 在代码旁边以实时示例展示每个组件。
- *Guides* 介绍时间与语言、安全、API、MCP、数据和主题。
- *Reference* 包含运行时协议和架构。

## 原则

- **性能优先。** 页面在服务器上渲染并以 HTML 流式输出。服务器上没有按用户保存的 UI 状态，也没有客户端框架。
- **无页面重载。** 导航、窗口、语言、时区和主题的变更都就地应用。
- **默认安全。** 每个 action 和界面都声明其访问控制。CSRF 令牌、严格的内容安全策略（Content Security Policy）以及对用户数据的转义始终开启。
- **每件事只有一种做法。** 语言很小，因此智能体编写的代码读起来就像人写的代码。
