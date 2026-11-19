#!/bin/sh
# Starts a new checkout or worktree with a warm Cargo target directory instead of a cold build.
# Cargo's fingerprints of registry dependencies do not depend on where the workspace is, so a
# copy of a built target directory (file times kept) is reused as it is: in a second worktree
# the first build of the showcase took 0.4 s after the copy against 191 s cold (2026-11-19).
#
#   scripts/warm-target.sh save [TEMPLATE]        build this checkout and keep its target as the template
#   scripts/warm-target.sh [TARGET] [TEMPLATE]    copy the template into TARGET (default ./target)
#
# TEMPLATE defaults to $ZEYN_TARGET_TEMPLATE or ../zeyn-target-template. Save again after
# Cargo.lock or the toolchain changes; a stale template only means more crates are rebuilt.
set -eu
features=assistant,oidc,totp,islands,realtime,socket-client,redis,xlsx,email,accounts,image,resumable,metronome-ops

if [ "${1:-}" = save ]; then
    template=${2:-${ZEYN_TARGET_TEMPLATE:-../zeyn-target-template}}
    rm -rf "$template"
    CARGO_TARGET_DIR=$template CARGO_INCREMENTAL=0 cargo build --workspace
    CARGO_TARGET_DIR=$template CARGO_INCREMENTAL=0 cargo test --workspace --no-run --features "$(echo "$features" | sed 's/[^,]*/zeyn\/&/g')"
    rustc -V > "$template/.warm-rustc"
    echo "$template: $(du -sh "$template" | cut -f1) saved"
    exit 0
fi

target=${1:-target}
template=${2:-${ZEYN_TARGET_TEMPLATE:-../zeyn-target-template}}
[ -d "$template" ] || { echo "no template at $template: run scripts/warm-target.sh save first" >&2; exit 1; }
[ ! -e "$target" ] || [ -z "$(ls -A "$target" 2>/dev/null)" ] || { echo "$target is not empty" >&2; exit 1; }
if [ -f "$template/.warm-rustc" ] && [ "$(cat "$template/.warm-rustc")" != "$(rustc -V)" ]; then
    echo "template was built by $(cat "$template/.warm-rustc"), this checkout uses $(rustc -V): save it again" >&2
    exit 1
fi
mkdir -p "$target"
cp -a "$template/." "$target/"
echo "$target: $(du -sh "$target" | cut -f1) copied from $template"
