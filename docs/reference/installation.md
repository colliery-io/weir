# Installation

weir is a single binary (`weir`) plus a set of **WASM connector** components it loads at runtime.

## Prerequisites

- **Rust 1.93+** (to build from source).
- The `wasm32-wasip2` target, to build the connectors: `rustup target add wasm32-wasip2`.
- **Docker** — optional, only for the Postgres destination in the tutorial and for the integration/soak stacks.

## Build from source

```bash
git clone https://github.com/colliery-io/weir.git
cd weir
(cd weir-ui && trunk build --release)  # embed the web UI (needs `cargo install trunk --locked`)
cargo install --path crates/weir-cli   # installs the `weir` binary
weir --help
```

Skipping the `trunk build` step still produces a working binary, but it serves a
placeholder page instead of the web UI — the UI is embedded at build time from
`weir-ui/dist`.

## Stage the connectors

Connectors ship as WASM components; weir loads them from a directory given by **`WEIR_CONNECTORS_DIR`**. Stage
the bundled connectors into a directory once:

```bash
bash scripts/stage-connectors.sh ./connectors
export WEIR_CONNECTORS_DIR="$PWD/connectors"
```

This stages every production connector plus the test fixtures — today: `rest`, `s3`, `rest-dest`,
`snowflake`, `postgres`, `mssql`, and the fixtures `arrow-sink`, `echo`, `slow`, `faulty`. The
authoritative list is the `stage` lines in `scripts/stage-connectors.sh` — check there if this
paragraph looks stale. Declarative (manifest) connectors are loaded from **`WEIR_MANIFESTS_DIR`** /
**`WEIR_DEST_MANIFESTS_DIR`** (the `manifests/` and `dest-manifests/` directories in the repo).

## Docker stacks (optional)

One `compose.yml`, two profiles:

```bash
# Demo: weir + its Postgres only → http://localhost:8080
docker compose --profile demo up --build      # or: angreal docker up

# Integration-test estate (Postgres, MSSQL, MinIO, Dex)
angreal integration up                        # = docker compose --profile integration up -d --wait
```

Host ports are overridable so the stack can coexist with services already on those ports:
`WEIR_HTTP_HOST_PORT` (weir UI/API, default 8080), `WEIR_PG_HOST_PORT` (5432, loopback-only),
`WEIR_MSSQL_HOST_PORT` (1433, loopback-only), `WEIR_MINIO_HOST_PORT` (9000, loopback-only).

## Runtime knobs

The daemons (`weir serve` / `weir api` / `weir runner`) retry **transient** run failures
(rate limits, network blips) with exponential backoff — 3 attempts by default. Override with
`WEIR_MAX_ATTEMPTS` (min 1) and `WEIR_RETRY_BASE_MS` (default 1000). Fatal errors —
bad config, connector-declared fatals — still fail immediately.

**Retention**: `weir serve` prunes finished run history, run logs, and dead letters on the
scheduler tick (leader-only) so the store never grows without bound — by age
(`WEIR_RETENTION_DAYS`, default 30) and by a per-tenant row cap per table
(`WEIR_RETENTION_MAX_ROWS`, default 10000, newest kept). Set either to `0` to disable that
cap. In-flight runs are never pruned; dead letters are purged, not replayed.

## First run

The API and UI are authenticated by default. On a **fresh store**, the first `weir api` (or
`weir serve`) mints the bootstrap admin key and prints it **once** — save it; restarts never
re-print it. To pre-mint instead, run `weir init` first. See
[Secure the control plane](../guides/secure-control-plane.md).

## Next

- Follow [Your first sync](../tutorials/first-sync.md) to run an ingest end to end.
- See the [HTTP API](../api/index.md) reference for the control plane.
