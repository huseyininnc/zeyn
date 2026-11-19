#!/bin/sh
# Reads one Rust source file on stdin, compiles it against the prebuilt libraries in /deps and
# runs it. The page goes to stdout; compiler messages and one "zeyn-play:" timing line go to
# stderr. Compiling may take 17 s and running 3 s; the host kills the container at 25 s.
set -u
cd /tmp
head -c 65536 > main.rs
now() { date +%s%3N; }
peak() { cat /sys/fs/cgroup/memory.peak 2>/dev/null || echo unknown; }
start=$(now)
if ! timeout -s KILL 17 rustc --edition 2024 --crate-type bin --crate-name play \
    -C opt-level=0 -C debuginfo=0 -C strip=symbols -C codegen-units=1 \
    -L dependency=/deps \
    --extern zeyn="$(echo /deps/libzeyn-*.rlib)" \
    --extern zeyn_playground_harness="$(echo /deps/libzeyn_playground_harness-*.rlib)" \
    -o /tmp/play main.rs 1>&2; then
  echo "zeyn-play: compile failed compile_ms=$(( $(now) - start )) memory_peak=$(peak)" >&2
  exit 3
fi
built=$(now)
timeout -s KILL 3 /tmp/play
status=$?
echo "zeyn-play: compile_ms=$(( built - start )) run_ms=$(( $(now) - built )) memory_peak=$(peak)" >&2
exit $status
