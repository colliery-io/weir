# Architecture

weir is one binary that plays several roles, over a shared **system of record**. Understanding the pieces — and
how a single sync moves through them — explains most of weir's behaviour.

## The pieces

- **Control plane / API** (`weir api`) — the HTTP surface and the embedded web UI. It's where connections,
  tenants, keys, and the catalog live, and where operators watch health. It also hosts the scheduler and a
  worker fleet in-process, so a single `weir api` is a complete node.
- **Store** — the durable state: connections, the connector catalog, per-stream cursors, the work-unit queue,
  run history, dead-letters, schemas. SQLite for single-node/dev; Postgres for anything shared or scaled.
- **Scheduler** — turns a connection's `every_secs`/`cron` into due work, enqueuing a **work unit**. Only the
  lease **leader** schedules, so many replicas are safe.
- **Orchestrator** — the work-unit queue plus the **worker fleet** that claims and executes units. Claims are
  leased, so many workers (and many nodes) drain the same queue without stepping on each other.
- **Engine** — the actual sync: it drives a source connector to read records, applies in-flight mapping and
  schema enforcement, and drives a destination connector to write them, committing progress at checkpoints.
- **WASM runtime** — loads and runs connectors as sandboxed `wasm32-wasip2` components, mediating their only
  outside reach (network egress) and injecting secrets on the way out.

## How a sync flows

1. A schedule (or an explicit `run`) enqueues a **work unit** for a connection under a tenant.
2. A worker **claims** the unit (a lease), and hands it to the engine.
3. The engine loads the source + destination connectors and streams records: **read → map → enforce schema →
   write**, in batches.
4. At each **checkpoint** the engine commits — the cursor advances, an outbox row lands, dead-letters flush —
   all in one transaction. If the process dies mid-run, the next claim resumes from the last checkpoint.
5. The run finishes `done` (or `failed` with a reason); the UI and `/overview` reflect it.

## Why it's shaped this way

The split between an **async orchestration boundary** (scheduling, leasing, the queue) and a **synchronous
execution core** (the engine, one unit at a time) keeps the hard part — correct, resumable data movement —
simple and deterministic, while the distribution concerns (concurrency, HA, per-tenant isolation) live in the
queue around it. Connectors are WASM so that untrusted third-party code can be run safely; see
[The connector model](connector-model.md). Delivery is checkpoint-based so that at-least-once + idempotent
writes give effectively-once results; see [Delivery guarantees](delivery-guarantees.md).
