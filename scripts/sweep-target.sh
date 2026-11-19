#!/bin/sh
# Keeps a Cargo target directory in bounds: removes it when it holds more than LIMIT_GB
# (default 20) or when it was built by another rustc than the one this repo pins.
# Cargo never deletes artifacts of old toolchains, feature sets or branches, so without
# this the directory only grows. The next build starts clean. Run it when no cargo
# build uses the directory, for example once a day before work.
#
#   scripts/sweep-target.sh [LIMIT_GB] [TARGET_DIR]
set -eu
limit=${1:-20}
target=${2:-${CARGO_TARGET_DIR:-target}}
[ -d "$target" ] || exit 0
[ -f "$target/.rustc_info.json" ] || [ -f "$target/.sweep-rustc" ] || { echo "$target is not a Cargo target directory" >&2; exit 1; }

stamp="$target/.sweep-rustc"
rustc=$(rustc -V)
size=$(($(du -sk "$target" | cut -f1) / 1048576))

if [ "$size" -gt "$limit" ]; then
    reason="$size GB is over the $limit GB limit"
elif [ -f "$stamp" ] && [ "$(cat "$stamp")" != "$rustc" ]; then
    reason="it was built by $(cat "$stamp")"
else
    [ -f "$stamp" ] || echo "$rustc" > "$stamp"
    echo "$target: $size GB, kept"
    exit 0
fi

rm -rf "$target"
mkdir -p "$target"
echo "$rustc" > "$target/.sweep-rustc"
echo "$target: removed, $reason"
