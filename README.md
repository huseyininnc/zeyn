# Zeyn Framework

Zeyn is an open source Rust web framework for data heavy applications: grids, forms and workflows. AI agents write the application code. People read it, review it and change it when they need to.

Status: early development, not published. Everything below compiles and is tested.

## Try it

```
cargo run -p zeyn-example-showcase     # http://127.0.0.1:3002, every component and feature
cargo run -p zeyn-example-big-grid     # http://127.0.0.1:3000, one million rows
cargo run -p zeyn-example-billing      # http://127.0.0.1:3004, an app using the shared-domain library
cargo run -p zeyn-example-docs         # http://127.0.0.1:3008, the documentation site, written with Zeyn
cargo run -p zeyn-example-adisyon --features desktop   # the point of sale app as a desktop window
cargo run -p zeyn-cli -- new my-app    # a new project with the standard feature structure
cargo run -p zeyn-cli -- new my-domain --lib   # a library of features shared by several apps
```

## What works today

- Models, entities, actions and screens as attribute macros, with typed fields and compile time checks
- Server rendered screens with a small browser runtime; no JavaScript to write
- Data grid: server paging, infinite scroll, virtual scrolling over a million rows, filters, sorting, totals
- Forms for every field type, validation shared by server, client and API, row version conflict checks
- PostgreSQL and Oracle (11g and later) data layer behind features: grid queries in SQL, keyset
  cursors, capped counts, totals, transactions, streams and migrations (see `docs/database.md`)
- Dates and times without time zone bugs: `Date`, `Time`, `DateTime` (UTC), `LocalDateTime`, every
  IANA time zone with its history built in (see `docs/operations.md`)
- English and Turkish built in, compile time checked translation keys, locale aware money and dates
- Security: permissions, guards, interceptors, audit log, CSRF, CSP, rate limits, IP rules, CORS
- Single sign on through a proxy, tenants, licenses, layered configuration and secrets
- Operations: ed25519 signed licenses, Redis for several servers, `tracing` spans and optional
  OpenTelemetry export (see `docs/operations.md`)
- JSON API with OpenAPI 3.1 and problem details, optional API docs with basic auth or permissions
- MCP for agents: a project MCP server (`zeyn mcp`) and in app tools through WebMCP and `/_zeyn/mcp`, opt in with `App::mcp`
- An embedded AI assistant (feature `assistant`): any LLM, speech, pluggable history, confirmation before writes (see `docs/assistant.md`)
- Themeable design system with light and dark themes (see `docs/theming.md`)
- Field level audit, read audit with reasons, field masking, safe deploys, health checks, idle
  timeout and re authentication, CSV import, background jobs and approval workflows (see `docs/enterprise.md`)
- Apps and shared libraries in separate repositories, GitLab CI templates (see `docs/polyrepo.md`)
- Desktop apps: one executable in the system WebView, offline, with a guarded loopback server
  (`zeyn-desktop`, `zeyn build-desktop`, see `docs/desktop.md`)

## Goals

- Strong data grid that handles large datasets, in the browser or on the server
- Hybrid rendering: server driven pages by default, WebAssembly islands for heavy components
- Built in MCP server and llms.txt so agents can query components, validate screens and run tests
- Secure defaults: CSRF, CSP, role based access and audit logging in the core
- Predictable memory use and a single binary deployment
- A modern default UI kit, with headless components so teams can ship their own kit

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution you submit for inclusion in this project, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.
