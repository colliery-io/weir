---
id: change-record-contract-changeop
level: task
title: "Change-record contract (ChangeOp + RecordBatch::Changes)"
short_code: "WEIR-T-0113"
created_at: 2026-07-07T11:34:11.391379+00:00
updated_at: 2026-07-07T11:51:41.656159+00:00
parent: WEIR-I-0026
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0026
---

# Change-record contract (ChangeOp + RecordBatch::Changes)

## Parent Initiative

[[WEIR-I-0026]] — the contract every other task builds on.

## Objective

Add a **structured op** to the record path: a `ChangeOp { Insert, Update, Delete }` + a
`RecordBatch::Changes(Vec<ChangeRecord>)` variant (`ChangeRecord { op, data }`, `data` = the JSON row incl. key
columns). Flow it through the engine + mapping without disturbing the existing `Rows` path.

## Reference

- `crates/weir-connector-types/src/types.rs` — the host-side `RecordBatch` / `WriteMode` enums.
- The **shared WIT-type block** at the top of every connector `src/lib.rs` (echo, slow, faulty, arrow-sink, rest,
  rest-dest, postgres, s3) + `wasm-fixtures/*` — `RecordBatch` is defined there too; all must add the variant.
- `crates/weir-engine/src/lib.rs` — where `RecordBatch` is read/written; `mapping.rs` — apply to a change's `data`.
- `RecordBatch` derives serde (bincode across the wasm boundary) — a new enum variant is wire-compatible.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `ChangeOp` + `RecordBatch::Changes(Vec<ChangeRecord>)` in `weir-connector-types` + the shared block in all
  8 connectors; host + all wasm connectors build.
- [x] Engine `map_batch` maps a `Changes` batch's `data` (op preserved); `row_count_hint` + arrow-sink count a
  change as a row; rest-dest/postgres take a data-as-rows stub (op-aware apply → T-0115/T-0116).
- [x] Additive variant → wire-compatible; `contract_version` stays 1 (all connectors rebuilt in-tree).
- [x] Unit test (mapping over a `Changes` batch: op preserved, data transformed); clippy clean.

## Status Updates

### 2026-07-07 — done (`043c9a4`)

`ChangeOp` + `ChangeRecord` + `RecordBatch::Changes` added to `weir-connector-types` **and the shared block in
all 8 connectors**. Engine `map_batch` maps each change's `data` (rename/cast hits the row + key), **op
preserved**. Dest matches extended — arrow-sink counts; **rest-dest + postgres take a data-as-rows stub** (real
op-aware apply is [[WEIR-T-0115]]/[[WEIR-T-0116]]). Additive → wire-compatible; `contract_version` stays 1. Host
+ all 8 wasm connectors build; `mapping_transforms_a_changes_batch_preserving_op` green; clippy clean. **Complete.**
