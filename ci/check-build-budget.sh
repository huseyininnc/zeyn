#!/bin/sh
# Fails when the workspace build, the one line rebuild of crates/zeyn or the target folder is over
# its budget in ci/build-budget.txt (the value plus tolerance_percent).
# Usage: sh ci/check-build-budget.sh <build seconds> <rebuild seconds> <target megabytes> [budget file]
set -eu
build="${1:?usage: check-build-budget.sh <build s> <rebuild s> <target MB> [budget file]}"
rebuild="${2:?usage: check-build-budget.sh <build s> <rebuild s> <target MB> [budget file]}"
target="${3:?usage: check-build-budget.sh <build s> <rebuild s> <target MB> [budget file]}"
file="${4:-$(dirname "$0")/build-budget.txt}"

value() {
  found="$(sed -n "s/^$1=//p" "$file" | tr -d ' \r\n')"
  case "$found" in
    ''|*[!0-9]*) echo "$file: $1 must be a whole number, found '$found'" >&2; exit 2 ;;
  esac
  echo "$found"
}

tolerance="$(value tolerance_percent)"
failed=0

check() {
  name="$1"; unit="$2"; took="$3"; budget="$(value "$4")"
  case "$took" in
    ''|*[!0-9]*) echo "$name must be a whole number, got '$took'" >&2; exit 2 ;;
  esac
  limit=$(( budget * (100 + tolerance) / 100 ))
  echo "$name: ${took} ${unit}, budget ${budget} ${unit} plus ${tolerance} percent = ${limit} ${unit}"
  if [ "$took" -gt "$limit" ]; then
    echo "over the budget by $((took - limit)) ${unit}: find the cause in the cargo-timings artifact, or raise $4 in ci/build-budget.txt in the same change and say why in the commit" >&2
    failed=1
  fi
}

check "workspace build" s "$build" build_seconds
check "one line rebuild of zeyn" s "$rebuild" rebuild_seconds
check "target folder" MB "$target" target_megabytes
exit "$failed"
