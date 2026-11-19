# Contributing to Zeyn

Thank you for helping. This guide covers how to build the project, how branches and pull requests work, and what makes a change finished. [AGENTS.md](AGENTS.md) holds the full rules and is the source of truth; people and AI agents follow the same ones.

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md). Report security problems privately as described in [SECURITY.md](SECURITY.md), never in a public issue.

## Build and test

```
cargo build --workspace
cargo test --workspace
cargo test -p zeyn --test all grid::
cargo test -p zeyn --features assistant,oidc,totp,islands,realtime,xlsx,email,accounts
cargo check -p zeyn --all-features --tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
sh ci/clippy-features.sh
```

- The integration tests of `zeyn` are one binary (`crates/zeyn/tests/all/`, one module per file); a new test file is added there with a `mod` line in `main.rs`. Tests that install a counting global allocator or a process wide subscriber keep their own file under `crates/zeyn/tests/`.
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

A new worktree or checkout does not need a cold build. `scripts/warm-target.sh save` builds this checkout once (workspace and tests with the full feature list) into a template directory, `../zeyn-target-template` by default or `$ZEYN_TARGET_TEMPLATE`; `scripts/warm-target.sh [TARGET]` copies it into a new, empty target directory with file times kept, so Cargo reuses every dependency. Measured on 2026-11-19: the first showcase build in a second worktree took 0.4 s after the copy against 191 s cold; a copy that does not keep file times rebuilt 118 crates (123 s). Save the template again after `Cargo.lock` or the toolchain changes; the script refuses a template built by another rustc. sccache does not help here: its keys include the target path, so a second worktree had no Rust cache hits.

The parallel compiler front end is not stable yet. On a one line rebuild of `crates/zeyn` it cut 20.9 s to 15.0 s in one measurement (`RUSTC_BOOTSTRAP=1 RUSTFLAGS=-Zthreads=8`); changing `RUSTFLAGS` rebuilds every dependency, and the flag needs a nightly feature, so the repository does not set it. Use it in a terminal of your own if you want it.

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

### Where the build time goes

`crates/zeyn` is one crate of about 99,000 lines, and every example waits for it. Measured on 2026-11-19 on Windows (20 cores, `CARGO_BUILD_JOBS=12`, `CARGO_INCREMENTAL=0` as in CI, the dev profile above), median of three runs. A one line rebuild edits a doc comment in `crates/zeyn/src/lib.rs`. The `DataGrid` column was measured in a later session; it gave 12.9 s and 26.8 s for the one line rebuilds just before the change.

| Build | Before | After `Form` | After `DataGrid` |
|---|---|---|---|
| `cargo build -p zeyn`, one line rebuild | 13.3 s | 12.0 s | 12.4 s |
| `cargo build -p zeyn-example-showcase`, one line rebuild | 28.7 s | 25.9 s | 25.2 s |
| of which `zeyn` with the showcase features | 21.8 s | 19.9 s | not measured |
| of which the showcase binary | 6.2 s | 5.7 s | not measured |
| `cargo build --workspace` from an empty target | 167 s | 158 s | not measured |
| size of `target/` after it | 2483 MB | 2027 MB | not measured |
| LLVM IR lines of `zeyn` with the showcase features (`cargo llvm-lines`) | 1,758,032 | 1,623,967 | 1,584,250 |
| LLVM IR lines of the showcase binary | 886,195 | 716,911 | 501,713 |

Where `rustc` spends the time on `zeyn` with the showcase features (`RUSTC_BOOTSTRAP=1 cargo rustc -p zeyn --lib --features ... -- -Ztime-passes`, before the changes; 26.8 s in all, the codegen passes overlap):

| Pass | Seconds |
|---|---|
| LLVM passes | 7.9 |
| `generate_crate_metadata` (the `.rmeta` dependents wait for) | 6.4 |
| monomorphization collector | 5.4 |
| codegen to LLVM IR | 5.0 |
| type check | 4.0 |
| borrow check | 3.8 |
| macro expansion | 2.2 |
| coherence check | 1.8 |
| finishing codegen | 1.8 |
| link (rlib) | 1.0 |

No single function dominates: the largest is 0.2 percent of the IR. Generic types instantiated once per model do: in the showcase binary `DataGrid<E>` was 25 percent of the IR, `Form<M>` 16 percent and `SubmitForm<M, A>` 5 percent; a `Form` cost about 8,600 lines for each model. The changes so far:

- `Form<M>` keeps only the model in its generic part. Its fields read values through `&dyn Any`, and rendering, reading, picker checks, the schema and the tool schema live in a private `FormBody` that is compiled once. `Form` lines in the showcase binary: 139,865 to 30,123.
- The AI tool schema of a `DataGrid` is a plain function (2,698 lines for each row type before).
- `DataGrid<E>` keeps only the runners of inline edits and new rows in its generic part. Its columns, row styles, row actions, menus and details read the row as `&dyn Any`, and rendering, views, queries, paging, exports and the schema live in a private `GridBody` that is compiled once. Lines of functions named `DataGrid<..>` in the showcase binary: 221,702 to 15,045; the binary 716,911 to 501,713 and `zeyn` with the showcase features 1,623,967 to 1,584,250. One line rebuild (median of three, same machine): the showcase 26.8 to 25.2 s, `zeyn` 12.9 to 12.4 s.
- `[profile.dev.package.zeyn] debug = false` drops the line tables of the framework crate: 1.4 s off its one line rebuild and its rlib from 116 to 92 MB. Panic messages still name the file and line; a backtrace shows the framework's functions without lines. For a debugging session inside the framework, build with `--config 'profile.dev.package.zeyn.debug="line-tables-only"'` or `"full"`.

The crate is not split. The modules that do not depend on the rest (`time`, `ui/sanitize`, `ui/markdown`, `ui/barcode`, `ui/code`, `metronome/cron`, `i18n`) are about 6,000 lines, so a crate of them would take at most about 6 percent off a rebuild after a change elsewhere, while a change in them would still rebuild `zeyn`. Everything else depends on everything: `server` on all of it, and `screen`, `ui`, `grid` and `form` on each other through `Component`, `Render` and `Node`. A split along those lines needs a trait crate with re-exports for every public path, and in a clean build `zeyn` already starts late because of its dependencies (at 123 s of 167 s), not because of its size. The parallel front end (`-Zthreads=8`, 20.9 to 15.0 s for a one line rebuild) is the bigger lever once it is stable.

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
