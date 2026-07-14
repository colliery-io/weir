---
id: postgres-dest-applies-insert
level: task
title: "Postgres dest applies insert/update/delete"
short_code: "WEIR-T-0115"
created_at: 2026-07-07T11:34:13.898803+00:00
updated_at: 2026-07-07T14:23:28.821364+00:00
parent: WEIR-I-0026
blocked_by: [WEIR-T-0113]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0026
---

# Postgres dest applies insert/update/delete

## Parent Initiative

[[WEIR-I-0026]] — the consumer: deletes land in Postgres.

## Objective

The postgres destination honors a change's op: **upsert** on Insert/Update, and on **Delete** either a **hard
delete by key** or a **soft tombstone** — chosen per connection. Plain `Rows` batches write exactly as today.

## Reference

- `crates/connectors/postgres/src/lib.rs` — `write_inner` (Append/Overwrite/Upsert; Upsert already keys on
  `business_keys` with `ON CONFLICT DO UPDATE`). `key_text`/`lit`/`quote_ident` helpers.
- Delete semantics config (from [[WEIR-I-0026]]): a connection setting `on_delete = hard | tombstone`
  (+ tombstone column name, default `_deleted_at`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `write()` dispatches `Changes` → `apply_changes`: Insert/Update → upsert by `business_keys`; **Delete** →
  hard `DELETE` or tombstone `UPDATE`, per config.
- [x] Applied in **chunk order** (one statement per change, in sequence) so I→U→D ends deleted/tombstoned.
- [x] Config `on_delete` (hard|tombstone) + `tombstone_column` (default `_deleted_at`); requires `WriteMode::Upsert`
  keys (error otherwise).
- [x] Plain `Rows` unchanged (`write_inner`); a malformed change dead-letters, the batch continues.
- [x] Unit test on the emitted SQL per op+mode (`pg_cdc` builders in connector-types); workspace + wasm + clippy clean.

## Status Updates

### 2026-07-07 — done (`9832af7`)

`write()` collects `Changes` separately → `apply_changes` (op-aware, in order): upsert on Insert/Update; on
Delete a hard `DELETE FROM t WHERE key=…` or a tombstone `UPDATE SET _deleted_at = now()` (per `on_delete`
config; table gets the tombstone column via `ADD COLUMN IF NOT EXISTS`). Requires `WriteMode::Upsert`
business_keys. Malformed change → dead-letter, batch continues. Pure SQL builders `pg_cdc::{upsert,delete,
tombstone}` in `weir-connector-types` with a unit test (the connector's fidius macros preclude `cargo test`).
Workspace + postgres wasm build; clippy clean. **Complete.**
