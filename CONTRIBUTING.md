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
sh ci/clippy-features.sh
```

- Tests that need PostgreSQL, Oracle, Redis or an identity provider skip unless `ZEYN_TEST_PG`, `ZEYN_TEST_ORACLE`, `ZEYN_TEST_REDIS` or `ZEYN_TEST_OIDC` is set. Each test file says how to start the server in Docker.
- Disk space can be tight. Use `CARGO_INCREMENTAL=0`, and delete your own `target` directory when you are done. The `pdf` feature links Typst (about 40 MB); do not turn it on for the whole workspace.
- `cargo test` does not rebuild a binary you then run by hand. Run `cargo build -p <example>` first.
- The workspace clippy run unifies Cargo features, so an unused import or item in a smaller feature set stays hidden. `sh ci/clippy-features.sh` runs clippy on the `zeyn` library with no default features, with each feature that builds without `server` (`signed-licenses`, `postgres`, `oracle`, `xlsx`, `markdown`, `lang-all`) and with the default features; CI runs it too. Code that only the `server` feature reads gets `#[cfg(feature = "server")]`, not `#[allow]`.
- Run an example to see a change: `cargo run -p zeyn-example-showcase` (http://127.0.0.1:3002), `cargo run -p zeyn-example-docs` (http://127.0.0.1:3008) or `cargo run -p zeyn-example-components` (http://127.0.0.1:3010).

### Build speed and disk use

The workspace `Cargo.toml` builds dependencies with `opt-level = 1` and no debug info, and workspace crates with line tables only: backtraces show file and line, a debugger shows no local variables. Set `CARGO_PROFILE_DEV_DEBUG=full` for a debugging session. On Windows `.cargo/config.toml` links with `rust-lld`, which ships with the toolchain.

Cargo never deletes old artifacts, so `target` only grows: every toolchain, feature set and branch adds its own copies. `scripts/sweep-target.sh` removes the directory when it is over a size limit (20 GB by default) or was built by another toolchain than the one in `rust-toolchain.toml`. Run it once a day while nothing builds, from Git Bash on Windows:

```
sh scripts/sweep-target.sh          20 GB limit, ./target or $CARGO_TARGET_DIR
sh scripts/sweep-target.sh 10 D:/zeyn-target
```

Several builds on one target directory wait for each other's lock, and several builds on separate directories at once can run out of memory and page file. When agents or terminals build in parallel, give each its own target directory and keep one build queue per directory. Lower the number of compiler jobs per build with `CARGO_BUILD_JOBS` (for example half the cores) when two builds run at the same time.

Windows Defender scans every file the compiler writes, which slows a build a lot. Two changes, both need an administrator, so make them yourself:

1. Exclude the build folders from real time scanning. In an administrator PowerShell:
   ```
   Add-MpPreference -ExclusionPath "$env:USERPROFILE\.cargo", "$env:USERPROFILE\.rustup", "C:\path\to\zeyn\target"
   ```
   Files in these folders are no longer scanned, so exclude only build output and the toolchain.
2. Or create a Dev Drive (Settings, System, Storage, Advanced storage settings, Disks and volumes, Create Dev Drive; at least 50 GB). Defender scans a Dev Drive in performance mode. Point the build there for your user: `setx CARGO_TARGET_DIR D:\zeyn-target`, and open a new terminal.

### Build time budget

CI builds the workspace with `cargo build --workspace --timings`, then touches `crates/zeyn/src/lib.rs` and rebuilds `zeyn` (the cost of a one line change), measures the size of `target/`, and fails when one of the three is over its budget in `ci/build-budget.txt` by more than `tolerance_percent` (`sh ci/check-build-budget.sh <build seconds> <rebuild seconds> <target megabytes>` is the check). The job uploads the report as the artifact `cargo-timings`; open `cargo-timing.html` to see which crate took the time. A change that makes a crate much slower to compile, or the target folder much larger, should bring the numbers back under the budget, or raise the budget in the same change and say why in the commit; a change that costs compile time states the before and after numbers in its commit. After the first CI runs on a new runner or toolchain, set the budgets to the measured numbers (the step summary prints them); the tolerance of 30 percent keeps noise from failing the job.

### Browser tests

The browser tests in `e2e/` open every page of the component catalog in Chromium and check
rendering, keys, visible focus, accessibility (axe), both themes, right to left, touch targets,
console errors, accessibility tree snapshots and size budgets. They need Node.js 20 or later:

```
cd e2e && npm ci && npx playwright install chromium && npm test
```

The suite starts the catalog with `cargo run -p zeyn-example-components` on port 3110 (set
`ZEYN_E2E_PORT` to change it) unless one already runs there. `npm run update` rewrites the
accessibility tree snapshots after an intended change. See
[docs/component-quality.md](docs/component-quality.md#4-browser-tests) for what each check does,
the budgets and the known issues list.

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
- **Breaking changes.** A breaking change to a public API adds a `zeyn migrate` step in the same pull request (`crates/zeyn-cli/src/migrate/`, one module per version): a rename, a new call shape or a moved path, or a review pattern with the reason when an automatic rewrite is not safe.
- **Tests.** New logic gets a test in the matching file under `crates/zeyn/tests/`. A security fix gets a regression test that fails without the fix.
- **Browser check.** Browser behavior is checked in a real browser (the showcase or the docs app) with no console errors.

If you change `skills/zeyn/SKILL.md`, copy it into `crates/zeyn-cli/embedded/` too. A test fails until you do.

## Documentation and translations

The docs are Zeyn pages under `examples/docs/pages/` plus guides in `docs/`. How macro errors point at the wrong token, and how that was measured, is in [docs/ide.md](docs/ide.md). Read the table in [AGENTS.md](AGENTS.md) to find the page for your area. When code and docs disagree, fix the wrong one in the same change.

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
