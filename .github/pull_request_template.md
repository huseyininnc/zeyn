## What changed

## Why

## How it was checked

## Checklist

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] Browser check with no console errors, if the UI changed
- [ ] Docs and translations updated, hashes recorded with `ZEYN_DOCS_TRANSLATED=all cargo test -p zeyn-example-docs record_translations`
- [ ] No new physical CSS directions (use logical properties such as `inline-start`)
- [ ] Security, performance and accessibility rules in AGENTS.md followed
