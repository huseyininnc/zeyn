# syntax=docker/dockerfile:1.7
# The sandbox image of the playground: the Rust toolchain plus zeyn and the harness compiled
# once, so a run compiles one small crate. Build from the repository root:
#   docker build -f tools/playground/sandbox/Dockerfile -t zeyn-playground-sandbox .

FROM rust:1.98-slim-bookworm AS build
WORKDIR /src
COPY . .
ENV CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build -p zeyn-playground-harness \
    && mkdir /deps \
    && cp target/debug/deps/*.rlib target/debug/deps/*.so /deps/

FROM rust:1.98-slim-bookworm
COPY --from=build /deps /deps
COPY tools/playground/sandbox/run.sh /usr/local/bin/zeyn-play
RUN chmod 0555 /usr/local/bin/zeyn-play
ENV TMPDIR=/tmp
USER 65534:65534
WORKDIR /tmp
ENTRYPOINT ["/usr/local/bin/zeyn-play"]
