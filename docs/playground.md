# Playground: design note

A browser playground lets a person edit the body of a screen function and see the screen
rendered. The code is Rust, so something has to compile it. This note compares three ways to do
that, lists the abuse cases, and recommends one. A local prototype of the recommendation lives in
`tools/playground`; nothing here ships in the `zeyn` crate.

The fixed rule: no code a visitor wrote is compiled or run on the servers that serve the docs or
any other site. Compiling Rust runs code (procedural macros, `build.rs`, `const fn`), so a compile
is as dangerous as a run.

## What a visitor writes

Only the body of one function. The playground wraps it:

```rust
use zeyn::prelude::*;

#[screen(route = "/", public)]
pub fn playground() -> Screen {
    // the visitor's code
}

fn main() {
    zeyn_playground_harness::print_page(
        App::new("Playground")
            .home(playground)
            .feature(Feature::new("playground").screen(playground)),
    );
}
```

`print_page` renders `/` in memory through `zeyn::testing::TestApp` (no port is opened) and prints
the page HTML. The result of a run is that HTML and nothing else.

## Options

### A. Compile in throwaway containers on a separate sandbox host

A sandbox host, not the docs host, runs one container per edit from a prepared image that holds
the Rust toolchain and `zeyn` with its dependencies already compiled. A run compiles one file
against those libraries and runs it.

Measured on the prototype (Docker 29.9 under Docker Desktop on WSL 2, 8 CPUs given to the VM, Rust 1.98.1):

| What | Number |
|---|---|
| Image build, one time (`zeyn` and its dependencies, debug) | 110 s, of which `cargo build` 69 s |
| Image size | 1.45 GB on disk, 361 MB compressed; the base `rust:1.98-slim-bookworm` is 1.23 GB, the prebuilt libraries in `/deps` are 172 MB (76 files) |
| Compile of a small screen (5 lines), warm | 200 to 245 ms (median 210 ms) |
| Compile of a medium screen (90 lines, 40 `format!` calls), warm | 250 to 510 ms (median 294 ms) |
| Compile after the host page cache was dropped (cold) | 630 ms small, 720 ms medium |
| Running the program (render the page) | 24 to 42 ms |
| Whole request, warm (container start, compile, run, HTML back) | 630 to 860 ms; container start and removal are about 450 ms of it |
| Whole request, cold | about 1.2 s |
| Memory peak of the container (cgroup `memory.peak`), warm | 102 MB small, 114 MB medium |
| Memory peak, cold | 700 MB, because reading the 172 MB of libraries and the toolchain into the page cache is charged to the first container |
| Page size | 6.3 KB for the small screen |

The trick that makes this fast is calling `rustc` directly, with `--extern zeyn=...` and
`-L dependency=/deps`, instead of `cargo`. Cargo would need a writable target directory and
would check fingerprints of every dependency; `rustc` only reads the prebuilt `.rlib` files and
writes the binary to `/tmp`. Rust 1.98 links with `rust-lld` by default, which keeps linking a
binary with axum and tokio under a few hundred milliseconds. Without the prebuilt libraries a
compile would build `zeyn` and its dependencies every time: 69 s with 8 CPUs.

### B. Compile to wasm32 and render in the browser

The core of `zeyn` (screens, components and rendering, without the `server` feature) compiles for
`wasm32-unknown-unknown`: `cargo check -p zeyn --no-default-features --target
wasm32-unknown-unknown` passes in 25 s. The page shell, `TestApp`, the router and the runtime
assets need `server` (axum and tokio) and do not. `crates/zeyn-island` already runs Rust in the
browser for islands.

This moves where the program runs, not where it is compiled. There is no supported build of
`rustc` that runs in a browser, so the compile still happens on a server, inside the same kind of
sandbox as option A. What option B saves is the run step (24 to 42 ms of the measured request)
and it adds a wasm download per edit (its size was not measured here),
a second render path without the page shell, and work to keep the two paths equal. It is worth
doing only if the playground later needs interactive previews that call back into Rust code
without a server round trip.

### C. Interpret a restricted screen DSL instead of compiling Rust

A small language (or JSON in the shape of `Component::schema`) interpreted by a trusted server
needs no sandbox for code: no compile, no processes, microseconds per edit, and almost no cost.
It also stops being Rust. Visitors would learn a second way to write a screen that does not
exist in a real app, every component would need an interpreter entry kept in step with its
builder, and closures, `format!`, iterators and types (the things that make a screen readable)
would be missing. The framework has one way to write a screen; a playground that teaches another
works against that.

### Recommendation

Option A. It shows real Rust with the real compiler messages, the per edit cost is under a
second, and the sandbox boundary is a container on a host that holds nothing else. Option B can
be added on top later for interactive previews; option C is not recommended.

## The sandbox

Every run is a new container, removed when it ends:

```
docker run --rm -i --network none --read-only --tmpfs /tmp:rw,exec,nosuid,nodev,size=256m
  --memory 1g --memory-swap 1g --cpus 2 --pids-limit 256 --cap-drop ALL
  --security-opt no-new-privileges --ulimit fsize=268435456 zeyn-playground-sandbox
```

- The source goes in on standard input; the page comes out on standard output (at most 1 MB is
  read) and compiler messages on standard error (at most 64 KB).
- The image runs as user 65534 with Docker's default seccomp profile. The root file system is read
  only; `/tmp` is a 256 MB tmpfs.
- Inside, `timeout -s KILL` stops the compile at 17 s and the run at 3 s, 20 s together. The host
  removes the container (`docker rm -f`) if it has not ended after 25 s.
- On a public host, add a second kernel boundary: run the containers under gVisor (`--runtime
  runsc`) or in a microVM (Firecracker, Kata). Docker's default isolation shares the host kernel.

## Abuse cases

Each case below was run in the prototype with the validator skipped, as if a visitor got past it.

| Case | What stops it | Measured |
|---|---|---|
| Endless loop at run time | `timeout -s KILL 3` | killed after 3.5 s, exit 137 |
| Endless `const` evaluation at compile time | rustc's `long_running_const_eval` lint; with the lint allowed, `timeout -s KILL 17` | error after 0.9 s; with the lint allowed, killed at 17.0 s |
| Type or monomorphization explosion at compile time | the 1 GB memory limit | killed after 2.2 s at 1 GB |
| Memory hog (allocate in a loop) | the 1 GB memory limit, no swap | killed after 0.5 s |
| Fork bomb (spawn processes in a loop) | `--pids-limit 256`, then the run timeout | held at the limit, killed after 3.5 s |
| Network egress (crypto mining pools, scanning, spam) | `--network none` | `Network is unreachable (os error 101)` |
| Disk fill | read only root, 256 MB tmpfs, `fsize` ulimit | `/etc` write: `Read-only file system`; `/tmp` stopped at 255 MB |
| Crypto mining with the CPU | 2 CPUs and 20 s per run, the per address rate limit, the queue | at most 20 s of 2 CPUs per run |
| Procedural macros that run code at compile time | a visitor cannot define one: it needs its own crate; only `zeyn`'s macros are linked | |
| `build.rs`, extra dependencies | there is no `Cargo.toml`: one file is compiled with a fixed `--extern` list | |
| `include_str!("/etc/passwd")` and other file reading macros | the validator; inside the container they would only see the image | |
| Escaping the function (`} fn main() { ... } fn x() {`) | the validator checks brackets with a small lexer that knows strings, raw strings, characters, lifetimes and comments | |
| Script in the rendered HTML | the preview origin's CSP (`script-src 'self'`, no inline script), escaping in every component | |
| Phishing or navigating the editor from the preview | a separate origin; the frame is sandboxed without `allow-top-navigation`, `allow-popups` or `allow-forms` | |

The validator (`tools/playground/src/check.rs`) is a convenience, not the boundary. It keeps the
rules: no attributes, no macros other than `t!`, `format!` and `vec!`, no `unsafe`, `extern` or
`mod`, at most 16 KB.

## What is returned and where it is served

Only the HTML of the rendered page. It is stored by the SHA-256 of the full source and served from
a separate origin that serves nothing but these pages and the zeyn runtime assets:

- The preview origin must be a separate registrable domain in production, not a subdomain or a
  port of the docs site, so it shares no cookies, storage or same site status with it. (In the
  prototype it is another port of 127.0.0.1, which separates the origin but not cookies.)
- The page carries `default-src 'none'; script-src 'self'; style-src 'self'; style-src-attr
  'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none';
  form-action 'none'; frame-ancestors <editor origin>`, plus `nosniff` and `no-referrer`.
- The editor frames it with `sandbox="allow-scripts allow-same-origin"`. The zeyn runtime reads
  cookies, which an opaque origin forbids, so `allow-same-origin` is needed; because the frame is
  cross origin, that flag gives it only its own origin, and the sandbox still blocks top
  navigation, popups, forms and downloads.
- The editor's own CSP lists only the preview origin in `frame-src`.

## Limits, queue and cache

| Control | Prototype value |
|---|---|
| Runs per address | 10 per minute; at most 10,000 addresses remembered |
| Containers at the same time | 2 (2 CPUs each) |
| Requests waiting for a container | 8; more get 503 |
| Pages cached by source hash | 256, oldest out first; a cached source answers in about 15 ms and is not counted against the rate limit |
| Largest body | 16 KB |

Behind a proxy the address comes from the proxy's header, and the limit should also apply per
network (for example per /64 for IPv6). A public deployment would add a proof of work or a
signed token per page view if one address range starts to fill the queue.

## Cost

A warm run takes about 0.25 s of compile plus 0.45 s of container start on 2 CPUs. A small VPS with
4 vCPUs and 8 GB of memory (about 7 to 15 EUR a month at common providers) runs 2 containers at
once: roughly 2.5 runs a second at full load, over 200,000 a day, or well under 0.001 EUR per
1,000 runs. The memory limit sets the number of slots: 1 GB per container plus the page cache of
the image (about 1.5 GB) fits 4 slots in 8 GB if the CPUs allow it. The cache by source hash makes
the examples in the docs free after their first run; they can also be rendered when the image is
built.

## Running the prototype

```
docker build -f tools/playground/sandbox/Dockerfile -t zeyn-playground-sandbox .
cargo run -p zeyn-playground
```

Open http://127.0.0.1:3120 (the editor); previews are served from http://127.0.0.1:3121.
`ZEYN_PLAYGROUND_EDITOR`, `ZEYN_PLAYGROUND_PREVIEW` and `ZEYN_PLAYGROUND_IMAGE` change the
addresses and the image. The image must be rebuilt when `zeyn` changes, since the host serves the
runtime assets of its own `zeyn` and the page links the assets of the image's `zeyn`.
