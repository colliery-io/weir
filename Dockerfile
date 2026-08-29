# syntax=docker/dockerfile:1
# Self-contained weir image: the host binary (Leptos UI embedded), pre-staged wasm
# connectors, and vendored declarative manifests — all built in this image and run
# in this image. "Build where you execute" ([[WEIR-A-0032]]) holds; the image is the
# unit, so nothing prebuilt crosses a machine boundary.
#
# Postgres is the production SoR; weir links libpq (kept) — and, today, libsqlite3
# too, because diesel-dualdb ([[WEIR-A-0009]]) carries both diesel backends as a
# hard dependency. libpq is glibc-linked, so a static/scratch image isn't reachable
# while we ship Postgres; the smallest *working* runtime is debian-slim + those libs
# + ca-certs. (Dropping libsqlite3 needs a `sqlite`-optional feature upstream in
# diesel-dualdb so weir-engine can build Postgres-only — then this image loses a lib
# and the sqlite default below.)

# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /src
# rust-toolchain.toml pins the toolchain AND its wasm targets — install it (plus trunk)
# before the source copy so the layer caches across source changes. A bare `rustup target
# add` against the image's default toolchain would miss the pinned one entirely.
COPY rust-toolchain.toml ./
RUN rustup toolchain install && cargo install trunk --locked
COPY . .
# UI → weir-ui/dist (embedded into weir-api by its build.rs), then the host binary.
RUN cd weir-ui && trunk build --release
RUN WEIR_REQUIRE_UI=1 cargo build -p weir-cli --release && strip target/release/weir
# Pre-stage the wasm connectors (built here = built where they run).
RUN bash scripts/stage-connectors.sh /out/connectors

# ---- runtime: slim glibc with weir's two dynamic libs + TLS roots ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      libpq5 libsqlite3-0 ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/weir /usr/local/bin/weir
COPY --from=build /out/connectors /app/connectors
COPY --from=build /src/manifests /app/manifests
COPY --from=build /src/dest-manifests /app/dest-manifests
ENV WEIR_CONNECTORS_DIR=/app/connectors \
    WEIR_MANIFESTS_DIR=/app/manifests \
    WEIR_DEST_MANIFESTS_DIR=/app/dest-manifests \
    WEIR_DB=/app/weir.db
WORKDIR /app
EXPOSE 8080
# Postgres-first: set WEIR_DB to a postgres:// URL for the production SoR
#   docker run -e WEIR_DB=postgres://user:pw@host/weir weir
# The sqlite default just lets a bare `docker run` come up for a demo. Schema is
# created on first open; the catalog populates via onboarding (manifests onboard
# instantly, connectors are pre-staged).
ENTRYPOINT ["/bin/sh", "-c", "exec weir --db \"$WEIR_DB\" api --port 8080"]
