---
id: postgres-cdc-source-emits
level: task
title: "Postgres CDC source emits structured changes"
short_code: "WEIR-T-0114"
created_at: 2026-07-07T11:34:12.410757+00:00
updated_at: 2026-07-07T13:57:15.691894+00:00
parent: WEIR-I-0026
blocked_by: [WEIR-T-0113]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0026
---

# Postgres CDC source emits structured changes

## Parent Initiative

[[WEIR-I-0026]] — the producer of change records.

## Objective

Upgrade the Postgres CDC read to emit `RecordBatch::Changes` (`ChangeRecord { op, data }`) parsed from
`test_decoding`, replacing the raw `{lsn, data}` text — so a delete carries its op + key downstream.

## Reference

- `crates/connectors/postgres/src/lib.rs` — `cdc_read` (currently `SELECT lsn, data FROM
  pg_logical_slot_get_changes`, pushes `{lsn, data}` text). The `test_decoding` line format is
  `table public.<t>: INSERT|UPDATE|DELETE: <col>[<type>]:<val> …`.
- [[WEIR-T-0107]] — the CDC harness (slot, REPLICA IDENTITY FULL so DELETE carries the key).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `cdc_read` parses each `test_decoding` line → a change (op + row JSON from `col[type]:val`; DELETE carries
  the key under REPLICA IDENTITY). `read()` emits `RecordBatch::Changes` for Cdc.
- [x] BEGIN/COMMIT skipped (don't start with `table `); the slot advances (cursor = last LSN) as before.
- [x] Non-CDC (`read_inner`) still emits `Rows` — unchanged.
- [x] Parser unit tests (2, in `weir-connector-types`); postgres wasm + workspace build; clippy clean.

## Status Updates

### 2026-07-07 — done (`11ad3b7`)

`read()` branches on `Cdc` → `read_cdc` → `RecordBatch::Changes`; others stay `Rows`. The pure `test_decoding`
parser was placed in **`weir-connector-types`** (host-testable — the connector's fidius macros preclude
`cargo test`) returning `(ChangeOp, row-JSON)`; the guest maps to its WIT `ChangeOp`. Handles
INSERT/UPDATE(new-tuple under REPLICA IDENTITY FULL)/DELETE, `''`-escaped quoted strings, null/bool/numeric
coercion. **2 parser unit tests green**; postgres wasm + workspace build; clippy clean. **Complete.**
