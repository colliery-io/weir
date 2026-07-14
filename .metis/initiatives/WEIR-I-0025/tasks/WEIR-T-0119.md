---
id: record-path-schema-enforcement
level: task
title: "Record-path schema enforcement"
short_code: "WEIR-T-0119"
created_at: 2026-07-07T22:33:05.366078+00:00
updated_at: 2026-07-07T23:28:21.143807+00:00
parent: WEIR-I-0025
blocked_by: [WEIR-T-0118]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0025
---

# Record-path schema enforcement

## Parent Initiative

[[WEIR-I-0025]] — the schema actually *bites*.

## Objective

On the engine record path, **coerce** each record to its stream schema (via the mapping `Cast` machinery); an
uncoercible value or a missing **required** (non-nullable) field **dead-letters** that record with a clear reason.
Extra fields pass through; the batch continues.

## Reference

- `crates/weir-engine/src/lib.rs` — `map_batch` (mapping stage over `Rows`/`Changes`) + the `DeadLetter` path;
  add a schema-enforce stage that composes with mapping ([[WEIR-T-0108]]).
- `crates/weir-engine/src/mapping.rs` — `cast_value` (str/int/float/bool coercion + null-passthrough) to reuse.
- The `StreamSchema` from [[WEIR-T-0118]].

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `mapping::enforce(schema, rec) -> Mapped` (pure): coerce each field to its `FieldType` (reuse
  `cast_value`; `timestamp`→text, `json`→any); missing/null non-nullable → dead-letter; extras pass through.
- [x] Wired via `map_batch(spec, schema, …)` → `process_record` (mapping then enforce) for both `Rows` and
  `Changes` (op preserved), enforced on the **post-mapping** shape; fast path (no ops, no schema) unchanged.
- [x] Dead-letters carry the field + reason; the batch continues.
- [x] Unit tests (coerce / uncoercible / missing-required / null vs absent / extras / json+timestamp) + an
  integration test (boolean schema → Slow's ints all dead-letter, 0 written). Workspace + clippy clean.

## Status Updates

### 2026-07-07 — done (`ef15f21`)

`mapping::enforce` coerces each schema field via the `Cast` machinery (`timestamp`→text, `json`→accept-any) and
dead-letters an uncoercible value or a missing/null non-nullable field; extras pass through. `map_batch` gained
an `Option<&StreamSchema>` and runs mapping→enforce per record (`process_record`); the sync loads the stored
schema (`Store::stream_schema`) and both **captures and enforces on the post-mapping shape**, so the two align
even when mapping renames fields — first run captures, later runs enforce. 3 enforce units + a live-ish
integration test (all 3 boolean-schema violations dead-lettered, 0 written); full engine suite + clippy green.
**Complete.** Next: [[WEIR-T-0120]] evolution diffs the stored schema on re-discover.
