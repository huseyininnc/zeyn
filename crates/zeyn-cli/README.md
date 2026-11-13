# zeyn-cli

The `zeyn` command of the [Zeyn Framework](https://github.com/huseyininnc/zeyn), a Rust web framework for data heavy business apps that AI agents write and people review.

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

- `zeyn new` creates a project with its agent files (`AGENTS.md`, the Zeyn skill and `.mcp.json`).
- `zeyn g feature <name>` and `zeyn g screen <feature> <name>` add code in the project's layout.
- `zeyn check` compiles the app and reports problems with codes, reasons and fixes (`--json` for tools).
- `zeyn mcp` is an MCP server over stdio for coding agents: `zeyn_docs` (rules, language, protocol, and every docs page of this version through `search` and `page`), `zeyn_describe`, `zeyn_check`, `zeyn_test`, `zeyn_new_feature` and `zeyn_new_screen`.

Docs: https://docs.zeynsoft.com/start/cli

mcp-name: io.github.huseyininnc/zeyn

## License

MIT or Apache 2.0, at your option.
