# weir-postgres-wasm

A **Postgres source + destination connector** for [weir](https://github.com/colliery-io/weir), shipped
as a sandboxed **WASM component** (`wasm32-wasip2`). It speaks the Postgres wire protocol directly over
`fidius_guest::sockets::tcp` + `postgres-protocol` — a **sync, pure-Rust client with no libpq and no
tokio** — so it runs inside the wasm sandbox.

> **Status: alpha (`0.0.1`).** Interfaces may change before `0.1`.

## Capabilities

- **Source** — `full_refresh`, `incremental` (by cursor field), and **CDC** via
  `pg_logical_slot_get_changes`; key-shard partitions for parallel reads.
- **Destination** — `append`, `overwrite`, and `upsert` (by business keys).
- **Auth** — md5 and SCRAM-SHA-256.

## Configuration

Either a connection URL or discrete fields:

```json
{ "url": "postgres://user:password@host:5432/dbname", "table": "orders" }
```

| Field | Meaning |
|---|---|
| `url` | `postgres://user:pw@host:port/db` (or use the discrete fields below) |
| `host`, `port`, `user`, `password`, `dbname` | discrete connection params |
| `table` | target/source table (defaults to the stream name) |

## Egress

The connector's TCP egress is governed by a **host policy** — the guest never sees credentials beyond
what the host authorizes.

Licensed under Apache-2.0.
