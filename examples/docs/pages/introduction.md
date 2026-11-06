# Introduction

Zeyn is an open source Rust web framework for data heavy applications: grids, forms and workflows. AI agents write the application code. People read it, review it and change it when they need to.

This site is itself a Zeyn app. Every page is Markdown rendered once on the server, the menu on the left is the app menu, and the examples marked *Live example* are real components running on this server.

## For AI agents

If you are an AI agent and someone asked you to build an app with Zeyn, start with the CLI instead of writing the project by hand:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Then read `AGENTS.md` and `.claude/skills/zeyn/SKILL.md` in the new project, and run `zeyn check` and `cargo test` after every change. A plain text guide with every page of these docs is at [/llms.txt](/llms.txt).

## What an app looks like

An app is a set of features. A feature holds entities, actions and screens. Screens are rendered on the server; a small runtime in the browser applies changes, so there is no JavaScript to write.

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

The same three macros carry the whole language: `#[entity]` describes data, `#[action]` is server code with declared access, and `#[screen]` is a page or a window. The compiler checks that every field, text key and action fits.

## Try it

Start your own app:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Or run the examples from the repository:

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

The showcase listens on `127.0.0.1:3002` and shows every component with sample data. This documentation listens on `127.0.0.1:3008`; set `ZEYN_DOCS_SERVER__ADDR` to change it.

## Finding your way

- Press **Alt+Shift+P** to open the command palette. Type a word and it searches every page of this site on the server.
- *Getting started* covers the CLI and the structure of a project.
- *Language* covers entities, actions, screens, forms, the DataGrid and texts.
- *Components* shows each component with a live example next to its code.
- *Guides* covers time and languages, security, the API, MCP, data and theming.
- *Reference* has the runtime protocol and the architecture.
- *Project* has the source, the license, the tests and CI, and the security policy.

## Principles

- **Performance first.** Pages are rendered on the server and streamed as HTML. There is no per user UI state on the server and no client framework.
- **No page reloads.** Navigation, windows, language, time zone and theme changes are applied in place.
- **Secure defaults.** Every action and screen declares its access. CSRF tokens, a strict Content Security Policy and escaping of user data are always on.
- **One way to do each thing.** The language is small, so code written by an agent reads like code written by a person.

Zeyn is open source under the [MIT or Apache 2.0 license](project.md#license). See the [security policy](project.md#security), the [roadmap](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md) and the [changelog](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md).
