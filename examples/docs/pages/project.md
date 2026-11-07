# Source, license and security

Zeyn is open source. This page says where the code lives, under which license, how it is tested and how to report a security problem.

## Source

The code and the issues are on GitHub: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). The crates are published on [crates.io](https://crates.io/crates/zeyn) and their API reference is on [docs.rs](https://docs.rs/zeyn).

## License

Zeyn is dual licensed under the [MIT license](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) or the [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE), at your option (`MIT OR Apache-2.0` in `Cargo.toml`). Contributions are accepted under the same two licenses.

## Quality

The repository holds 767 test functions (`#[test]` and `#[tokio::test]` in the crates and examples, counted on 2026-11-06), plus the examples in the doc comments, which run as doctests.

Every pull request and every push to `dev` and `main` runs CI:

- `cargo fmt --all --check` for formatting;
- `cargo clippy --workspace --all-targets -- -D warnings`, so any lint fails the build;
- `cargo test --workspace`, the tests of every crate and example;
- `cargo test -p zeyn` with the features `assistant`, `oidc`, `totp`, `islands`, `realtime`, `xlsx`, `email` and `accounts`;
- `cargo test -p zeyn-cli`, which also builds the framework without its server feature;
- `cargo deny` for licenses, advisories and banned dependencies.

The results of every run are public on the [Actions page](https://github.com/huseyininnc/zeyn/actions).

## Security

- **Supported versions.** Zeyn is in alpha. Only the latest `0.0.x` release gets security fixes.
- **Reporting.** Report a vulnerability privately through GitHub Security Advisories: open the Security tab of the repository and choose "Report a vulnerability" ([direct link](https://github.com/huseyininnc/zeyn/security/advisories/new)). Never report it in a public issue, discussion or pull request.
- **Response.** A report is acknowledged within 5 working days. After the fix is released, a security advisory is published and the reporter is credited unless they prefer not to be named.

The full policy, with what to include in a report and what is in scope, is in [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md).

## More

- [Changelog](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): the changes of every release.
- [Roadmap](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): the next release and what is planned.
- [Contributing](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): how to build, test and send a change.
- [Code of conduct](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): how we work together.
- [Support](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): where to ask questions and report bugs.
