# syntax=docker/dockerfile:1.7
# Images of the public sites: `--target docs` (docs.zeynsoft.com), `--target admin`
# (framework.zeynsoft.com, the admin template) and `--target components` (components.zeynsoft.com). Build from the repository root: docker build -f deploy/Dockerfile .

FROM rust:1.98-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release -p zeyn-example-docs -p zeyn-example-admin -p zeyn-example-components \
    && mkdir -p /out \
    && cp target/release/zeyn-example-docs target/release/zeyn-example-admin \
       target/release/zeyn-example-components /out/

FROM gcr.io/distroless/cc-debian12:nonroot AS docs
COPY --from=build /out/zeyn-example-docs /app/docs
ENV ZEYN_DOCS_SERVER__ADDR=0.0.0.0:3008
EXPOSE 3008
USER nonroot
ENTRYPOINT ["/app/docs"]

FROM gcr.io/distroless/cc-debian12:nonroot AS admin
COPY --from=build /out/zeyn-example-admin /app/admin
COPY examples/admin/config /src/examples/admin/config
ENV ZEYN_ADMIN_SERVER__ADDR=0.0.0.0:3011
ENV ZEYN_ADMIN_AUTH__ROLE=Viewer
EXPOSE 3011
USER nonroot
ENTRYPOINT ["/app/admin"]

FROM gcr.io/distroless/cc-debian12:nonroot AS components
COPY --from=build /out/zeyn-example-components /app/components
ENV ZEYN_COMPONENTS_SERVER__ADDR=0.0.0.0:3010
EXPOSE 3010
USER nonroot
ENTRYPOINT ["/app/components"]
