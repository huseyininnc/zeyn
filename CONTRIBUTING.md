# Contributing to Zeyn

Thank you for helping. This guide covers how to build the project, how branches and pull requests work, and what makes a change finished. [AGENTS.md](AGENTS.md) holds the full rules and is the source of truth; people and AI agents follow the same ones.

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md). Report security problems privately as described in [SECURITY.md](SECURITY.md), never in a public issue.

## Build and test

```
cargo build --workspace
cargo test --workspace
cargo test -p zeyn --features assistant,oidc,totp,islands,realtime,xlsx,email,accounts
cargo check -p zeyn --all-features --tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

- Tests that need PostgreSQL, Oracle, Redis or an identity provider skip unless `ZEYN_TEST_PG`, `ZEYN_TEST_ORACLE`, `ZEYN_TEST_REDIS` or `ZEYN_TEST_OIDC` is set. Each test file says how to start the server in Docker.
- Disk space can be tight. Use `CARGO_INCREMENTAL=0`, and delete your own `target` directory when you are done. The `pdf` feature links Typst (about 40 MB); do not turn it on for the whole workspace.
- `cargo test` does not rebuild a binary you then run by hand. Run `cargo build -p <example>` first.
- Run an example to see a change: `cargo run -p zeyn-example-showcase` (http://127.0.0.1:3002), `cargo run -p zeyn-example-docs` (http://127.0.0.1:3008) or `cargo run -p zeyn-example-components` (http://127.0.0.1:3010).

## Branches and pull requests

- Create your work branch from `dev`.
- Open the pull request into `dev`. Pull requests into `dev` are squash merged, so the pull request title becomes the commit message.
- A release goes from `dev` to `main` through a pull request that is merged with a merge commit.
- Nobody pushes directly to `dev` or `main`.
- Keep one pull request to one piece of work. CI (format, clippy, tests, `cargo deny`) must pass.

## What makes a change done

The sections of [AGENTS.md](AGENTS.md) under "The rules" are not style preferences. A change that breaks one is not done. In short:

- **Performance first.** Stream instead of buffering, borrow instead of clone, no per user UI state on the server, a cap on everything that grows with traffic, heavy integrations behind a Cargo feature. Measure anything that costs memory, binary size or time, and give the numbers in the pull request.
- **No component reloads the page.** In app navigation goes through `nav.js`. No `location.reload()` or `location.assign()`.
- **Security.** Every action and screen declares access. Every `POST` endpoint calls `check_csrf`. User data in HTML goes through `render::escape`. The CSP stays strict. Secrets never go in code, logs or tests.
- **Accessibility and UI.** WCAG 2.2 AA, both themes, colors from theme tokens, every user facing text in the locale files in English and Turkish.
- **Code.** Everything in English. No inline `//` comments, use `///` and `//!` doc comments on items. `rustfmt.toml` sets `chain_width = 40`; do not raise it. Generated macro code uses `::zeyn::` paths. Follow the existing patterns instead of adding a second way to do something.
- **Tests.** New logic gets a test in the matching file under `crates/zeyn/tests/`. A security fix gets a regression test that fails without the fix.
- **Browser check.** Browser behavior is checked in a real browser (the showcase or the docs app) with no console errors.

If you change `skills/zeyn/SKILL.md`, copy it into `crates/zeyn-cli/embedded/` too. A test fails until you do.

## Documentation and translations

The docs are Zeyn pages under `examples/docs/pages/` plus guides in `docs/`. Read the table in [AGENTS.md](AGENTS.md) to find the page for your area. When code and docs disagree, fix the wrong one in the same change.

English is the source. The docs are also translated into Turkish, Chinese, Russian, Spanish, Korean, Japanese, Hindi and Arabic. When you change an English page, update its translations if you can. A page whose translation is out of date is shown in English with a note, so a missing translation does not block your change.

Each translated page records the hash of the English source it was written from. After you update translations, record the hashes:

```
ZEYN_DOCS_TRANSLATED=all cargo test -p zeyn-example-docs record_translations
```

Use a comma separated list such as `tr,zh` instead of `all` to record only some languages. Public prose uses no em dashes or en dashes.

## Commit messages

Say what was done in plain words. Start with the ticket id when there is one.

```
ZEYN-42 PDF list reports for grids
Rail hides its scrollbar and scrolls by mouse drag
```

Do not use type prefixes such as `feat:` or `fix:`. Do not put secrets in a commit message.

## Licensing of contributions

Zeyn is licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option. Unless you explicitly state otherwise, any contribution you submit for inclusion in the project, as defined in section 5 of the Apache-2.0 license, is dual licensed as `MIT OR Apache-2.0`, without any additional terms or conditions. There is no contributor license agreement to sign.

Only submit code you wrote or have the right to submit under these licenses. Do not copy code from projects with incompatible licenses.

## AI assisted contributions

AI assisted contributions are welcome. Zeyn is built for AI agents to write application code, and the repository has instructions for them in [AGENTS.md](AGENTS.md). The same review rules apply to every pull request, whoever or whatever wrote it. You are responsible for what you submit: read it, run it, and be able to explain it.
