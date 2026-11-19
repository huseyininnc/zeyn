#!/bin/sh
# Fails when the workspace build took longer than the budget in ci/build-budget.txt.
# Usage: sh ci/check-build-budget.sh <seconds the build took> [budget file]
set -eu
took="${1:?usage: check-build-budget.sh <seconds> [budget file]}"
file="${2:-$(dirname "$0")/build-budget.txt}"
budget="$(tr -d ' \r\n' < "$file")"
case "$budget$took" in
  ''|*[!0-9]*) echo "build budget and time must be whole seconds: budget '$budget', took '$took'" >&2; exit 2 ;;
esac
echo "workspace build took ${took} s, budget ${budget} s"
if [ "$took" -gt "$budget" ]; then
  echo "over the build budget by $((took - budget)) s: find the crate in the cargo-timings artifact, or raise ci/build-budget.txt in the same change and say why" >&2
  exit 1
fi
