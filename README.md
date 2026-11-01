# Zeyn Framework

Zeyn is an open source Rust web framework for business software. AI agents write the application code. People read it, review it and change it when they need to.

Status: design phase. There is no usable code yet.

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
