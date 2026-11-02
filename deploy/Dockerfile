# syntax=docker/dockerfile:1.7
# Images of the public sites: `--target docs` (docs.zeynsoft.com) and `--target showcase`
# (framework.zeynsoft.com). Build from the repository root: docker build -f deploy/Dockerfile .

FROM rust:1.98-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release -p zeyn-example-docs -p zeyn-example-showcase \
    && mkdir -p /out \
    && cp target/release/zeyn-example-docs target/release/zeyn-example-showcase /out/

FROM gcr.io/distroless/cc-debian12:nonroot AS docs
COPY --from=build /out/zeyn-example-docs /app/docs
ENV ZEYN_DOCS_SERVER__ADDR=0.0.0.0:3008
EXPOSE 3008
USER nonroot
ENTRYPOINT ["/app/docs"]

FROM gcr.io/distroless/cc-debian12:nonroot AS showcase
COPY --from=build /out/zeyn-example-showcase /app/showcase
COPY examples/showcase/media /src/examples/showcase/media
ENV NORTHWIND_SERVER__ADDR=0.0.0.0:3002
EXPOSE 3002
USER nonroot
ENTRYPOINT ["/app/showcase"]
