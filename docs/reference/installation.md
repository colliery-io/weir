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
cargo install --path crates/weir-cli   # installs the `weir` binary
weir --help
```

## Stage the connectors

Connectors ship as WASM components; weir loads them from a directory given by **`WEIR_CONNECTORS_DIR`**. Stage
the bundled connectors into a directory once:

```bash
bash scripts/stage-connectors.sh ./connectors
export WEIR_CONNECTORS_DIR="$PWD/connectors"
```

This stages the built-in connectors — `echo`, `slow`, `arrow-sink` (test/dev), `postgres`, `rest`, `rest-dest`.
Declarative (manifest) connectors are loaded from **`WEIR_MANIFESTS_DIR`** / **`WEIR_DEST_MANIFESTS_DIR`** (the
`manifests/` and `dest-manifests/` directories in the repo).

## Next

- Follow [Your first sync](../tutorials/first-sync.md) to run an ingest end to end.
- See the [HTTP API](../api/index.md) reference for the control plane.
