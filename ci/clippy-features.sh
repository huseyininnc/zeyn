#!/bin/sh
# Runs clippy on the zeyn library with the feature sets that change which code is built, so an
# import or item that is unused in one of them fails here. The workspace run unifies features and
# hides these. Usage: sh ci/clippy-features.sh
set -eu
for set in "" server signed-licenses postgres oracle xlsx markdown lang-all; do
  echo "clippy: zeyn --no-default-features --features '$set'"
  cargo clippy -p zeyn --lib --no-default-features --features "$set" -- -D warnings
done
echo "clippy: zeyn default features"
cargo clippy -p zeyn --lib -- -D warnings
