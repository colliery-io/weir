---
id: delete-propagation-cdc-deletes-to
level: initiative
title: "Delete propagation — CDC deletes to destination tombstones"
short_code: "WEIR-I-0026"
created_at: 2026-07-07T04:00:27.930380+00:00
updated_at: 2026-07-07T20:25:36.878252+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: delete-propagation-cdc-deletes-to
---

# Delete propagation — CDC deletes to destination tombstones

## Context

A correctness gap for mirror-style syncs. The Postgres CDC source **captures** deletes (I→U→D via the logical
slot, proven in [[WEIR-T-0107]]), but the destination write modes are only `Append` / `Upsert` / `Overwrite` —
there is **no delete path**, so a row deleted at the source lingues forever at the destination. This initiative
carries a delete from capture through to the destination.

## Goals

- **Represent a delete on the record path** — a record carries its op (insert/update/**delete**) from a CDC
  source, distinguishable from an upsert, keyed by the primary key.
- **A destination delete/tombstone semantics** — a new write behaviour (hard delete by key, and/or a soft-delete
  tombstone column) so deletes land; define which destinations do which.
- **End-to-end proof** — extend the CDC fidelity harness: delete a source row → it's removed (or tombstoned) at
  the destination, exactly once, in order.

## Non-Goals

- Non-CDC delete detection (full-refresh "diff to find deletes" is a heavier, separate strategy).

## Design decisions (2026-07-07, approved) — the thorough path

- **Op envelope**: a **structured op in the `RecordBatch` contract** (not a reserved `_op` field). Add a
  `ChangeOp { Insert, Update, Delete }` + a `Changes(Vec<ChangeRecord>)` variant (`ChangeRecord { op, data }`,
  `data` = the JSON row incl. key columns). A WIT-contract change across every connector's shared block +
  `weir-connector-types`; the engine passes it through; mapping applies to each change's `data`.
- **Delete semantics**: **configurable per connection** — hard delete by key **or** soft tombstone (a marker
  column / `deleted_at`). A connection setting picks which.
- **Dest scope**: **both** `postgres-dest` (SQL delete / tombstone) **and** `rest-dest` (HTTP DELETE, or a
  configured delete call, per key on `op:delete`).
- Ordering: deletes ride the same per-partition checkpoint ordering as upserts ([[WEIR-T-0107]] proved the CDC
  slot delivers I→U→D in commit order once); the writer applies a chunk in order.
- Grows this initiative to **L** (5 tasks). No new ADR — extends [[WEIR-A-0026]] (record path) + the connector
  contract.

## Proposed decomposition (for sign-off)

- **T-a — Change-record contract:** add `ChangeOp` + `RecordBatch::Changes(Vec<ChangeRecord>)` to
  `weir-connector-types` + the shared WIT block in every connector; engine passthrough; **mapping** applies to a
  change's `data`; `ArrowSink`/count dests treat a change as one row. (contract-version handling.)
- **T-b — Postgres CDC source emits changes:** parse `test_decoding` (INSERT/UPDATE/DELETE + columns) →
  `ChangeRecord { op, data }` with the key columns (REPLICA IDENTITY), replacing the raw `{lsn,data}` text.
- **T-c — Postgres dest applies i/u/d:** honor op — upsert on insert/update, **hard-delete or tombstone** on
  delete (per-connection config); non-change `Rows` unchanged.
- **T-d — rest-dest applies deletes:** on `op:delete`, issue the configured delete (HTTP DELETE by key / a
  delete endpoint); upsert-style writes otherwise.
- **T-e — end-to-end fidelity:** extend the harness — CDC I→U→D → the row appears, updates, then is
  **removed (hard) or tombstoned**, once, in order; both dests; hard + tombstone modes.

## Exit Criteria (draft)

- [ ] CDC deletes carry through the engine with their key + op.
- [ ] A destination delete/tombstone write mode applies them.
- [ ] Fidelity test: source delete → destination removal/tombstone, once, in order.
