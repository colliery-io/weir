---
id: incremental-cdc-fidelity-harness
level: task
title: "Incremental + CDC fidelity harness"
short_code: "WEIR-T-0107"
created_at: 2026-07-06T11:33:17.321114+00:00
updated_at: 2026-07-06T22:49:34.332196+00:00
parent: WEIR-I-0022
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0022
---

# Incremental + CDC fidelity harness

## Parent Initiative

[[WEIR-I-0022]] — closes the **incremental / CDC fidelity** partial.

## Objective

Prove — not assert — that incremental and CDC syncs are **correct**: the cursor advances, a resumed sync
picks up exactly where it left off, and the output has **no duplicates and no gaps** across restarts.

## Reference

- `crates/weir-connector-types/src/types.rs` — `SyncMode::{FullRefresh, Incremental}`, `WriteMode`.
- `crates/connectors/postgres/src/lib.rs` — `SyncMode::{FullRefresh, Incremental, Cdc}`, cursor field,
  `pg_logical_slot_get_changes`, key-shard partitions.
- `crates/weir-engine/tests/wasm_postgres_engine.rs` — the existing engine-level Postgres test to extend.
- `stream_state` (per `(tenant, connection, stream)`) holds the cursor/checkpoint.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Incremental resume** — `pg_wasm_incremental_resume_delta`: only new rows on resume, cursor advanced.
- [x] **No dupes / no gaps** — `pg_wasm_incremental_no_dupes_no_gaps`: 7 rows delivered exactly once across resumes.
- [x] **CDC ordering** — `pg_wasm_cdc_insert_update_delete_advances_in_order`: I→U→D captured in sequence, slot
  advances, replay double-delivers nothing.
- [x] **Partition checkpoint isolation** — `pg_wasm_partition_checkpoints_are_isolated`: 2 shards cover all rows
  once, checkpoint independently via per-shard `state_key`.
- [x] Runs in the ignored integration lane; workspace + clippy clean.

## Status Updates

### 2026-07-06 — WIP (`380cb06`), live run blocked on Docker

- **Incremental fidelity written + compile-verified**: `pg_wasm_incremental_resume_delta` (a resumed sync
  returns only the new rows — no re-emission past the persisted cursor) and `pg_wasm_incremental_no_dupes_no_gaps`
  (across interleaved grow+resume rounds, each distinct row is delivered exactly once — dupes→more, gaps→fewer).
  These join the existing roundtrip / upsert-idempotent / cursor-advance / CDC-insert tests.
- **Harness now port-flexible**: `WEIR_TEST_PG_URL` (default 5432) + compose host port is `WEIR_PG_HOST_PORT`-
  overridable, so weir's stack coexists with another Postgres on 5432 (didn't disturb the user's cloacina DB).
- **BLOCKED**: Docker Desktop crashed mid-task ("unable to start") — `open -a Docker` ×2 + `cargo clean` (freed
  45 GB) didn't revive it; needs a manual restart. Live run pending:
  `WEIR_PG_HOST_PORT=5433 angreal integration up` then `WEIR_TEST_PG_URL=postgres://weir:weir@localhost:5433/weir
  cargo test -p weir-engine --test wasm_postgres_engine -- --ignored --test-threads=1`.
- **Remaining before done**: run the suite live; then CDC I/U/D *ordering* + partition-checkpoint isolation
  (need raw-SQL mutation + a value-capturing sink — deeper infra; scope decision pending).

### 2026-07-06 — done (`4354732`), all 5 ACs proven live (8/8)

Docker recovered; ran the whole suite live against weir's own compose stack (host port 5433, via
`WEIR_PG_HOST_PORT`, so it never touched the cloacina Postgres on 5432). Added the deeper CDC infra (user
chose "build it now"):

- **CDC ordering** without a payload sink: a raw `postgres` client drives one mutation at a time, and each CDC
  read consumes exactly that change — so read-after-INSERT, read-after-UPDATE, read-after-DELETE prove the
  sequence *and* the slot advances; a final replay returns 0 (no double-delivery).
- **Partition isolation**: two key-shard partitions read disjoint slices (cover all 12 rows once — no
  overlap/gap) and checkpoint independently via distinct `SyncOptions.state_key`.

**8/8 on a fresh integration DB.** (Two `Append`-based tests fail only when run against a DB dirtied by repeated
local runs — CI's fresh DB is the contract.) clippy clean. **Complete — closes the incremental/CDC fidelity
partial in full.**
