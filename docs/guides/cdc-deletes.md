# Capture changes and propagate deletes

weir captures **change-data-capture** (CDC) from a Postgres source and propagates every op — including
**deletes** — to the destination. A source delete becomes either a **hard delete** or a **tombstone** at the
destination, in order, so an `insert → update → delete` on one key ends deleted, never resurrected.

**Goal:** run a CDC connection so source deletes land at the destination.

## Prerequisites

- A Postgres source with **logical replication** (`wal_level = logical`) and **`REPLICA IDENTITY FULL`** on the
  captured table (so `UPDATE`/`DELETE` carry the row's columns — the delete needs its key).
- A destination that applies changes by op: the `postgres` destination (hard delete or tombstone) or the
  `rest-dest` destination (an HTTP `DELETE`).

## 1. Create a CDC connection

Set the sync mode to `cdc`, the write mode to `upsert` with the row's **business keys**, and the delete
behaviour in the connector `config`:

```bash
weir --db weir.db connection add \
  --name cdc-orders \
  --source postgres \
  --dest rest-dest \
  --stream orders \
  --sync-mode cdc \
  --write-mode upsert \
  --business-keys id \
  --config '{
    "url": "postgres://weir:weir@db:5432/app",
    "base_url": "https://api.example.com",
    "path": "/orders/{{ record.id }}",
    "delete_path": "/orders/{{ record.id }}"
  }'
```

- The `postgres` source defaults to **TLS** (`sslmode=require`) — right for managed databases; append
  `?sslmode=disable` to the URL for a plaintext local/dev server, or `?sslmode=verify-full` (+ inline-PEM
  `sslrootcert`) for verified TLS.
- `--sync-mode cdc` makes the source read the logical-replication stream as structured changes.
- `--write-mode upsert --business-keys id` makes Insert/Update upsert by `id`, and gives Delete its key.
- The `config` is shared by source and destination; each reads the keys it understands (here the `postgres`
  source reads `url`, and `rest-dest` reads `base_url`/`path`/`delete_path`).

## 2. Choose the delete semantics

For a **postgres** destination, the delete behaviour is set in `config`:

| Key | Default | Meaning |
| --- | --- | --- |
| `on_delete` | `hard` | `hard` → `DELETE FROM t WHERE <keys> = …`; `tombstone` → `UPDATE t SET <tombstone_column> = now()`. |
| `tombstone_column` | `_deleted_at` | The column stamped when `on_delete = tombstone`. |

For a **rest-dest** destination, a delete issues an HTTP `DELETE` (or `delete_method`) to the `delete_path` URL.

## 3. Run it

The first run establishes the replication slot; subsequent runs stream the changes since:

```bash
weir --db weir.db run --name cdc-orders     # or let a schedule / `weir serve` drive it
```

**Done** when a row deleted at the source is gone (hard) or tombstoned at the destination after the next run.
Changes apply in order; a malformed change dead-letters without aborting the batch.

## Postgres → Postgres

The source and destination configure **independently** ([[WEIR-I-0029]]) — `--source-config` and `--dest-config`
override the shared `--config` per side — so a `postgres → postgres` CDC connection reads one table and writes
another (no loop):

```bash
weir --db weir.db connection add \
  --name cdc-pg --source postgres --dest postgres --stream orders \
  --sync-mode cdc --write-mode upsert --business-keys id \
  --config '{"url":"postgres://weir:weir@db:5432/app"}' \
  --source-config '{"table":"orders"}' \
  --dest-config '{"table":"orders_replica","on_delete":"tombstone"}'
```

Both sides share the `url` from `--config`; the source reads `orders`, the destination writes (and tombstones)
`orders_replica`.

## See also

- [Connection config](../reference/connection-config.md) for the mode fields and `on_delete`.
- The change contract (`RecordBatch::Changes`, `ChangeOp`) is in the [connector contract](../reference/connector-contract.md);
  why in-order replay-safe application is sound is in [Delivery guarantees](../explanation/delivery-guarantees.md).
