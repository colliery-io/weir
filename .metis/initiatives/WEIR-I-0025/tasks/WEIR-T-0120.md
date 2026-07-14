---
id: schema-evolution-additive-vs
level: task
title: "Schema evolution — additive vs breaking"
short_code: "WEIR-T-0120"
created_at: 2026-07-07T22:33:06.881672+00:00
updated_at: 2026-07-07T23:39:00.230605+00:00
parent: WEIR-I-0025
blocked_by: [WEIR-T-0118]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0025
---

# Schema evolution — additive vs breaking

## Parent Initiative

[[WEIR-I-0025]] — handle a source that changes shape.

## Objective

When a stream's schema drifts, do the right thing: **auto-accept additive** (a new nullable field flows through
+ updates the stored schema); **detect + block/flag breaking** (a type change or a dropped required field) as a
run-level error surfaced to the operator — never silently applied.

## Reference

- The stored `StreamSchema` + capture ([[WEIR-T-0118]]); the enforcement stage ([[WEIR-T-0119]]).
- `crates/weir-app/src/lib.rs` — where a run re-discovers / resolves the stream; the run/error surface
  (`RunRow.error`, dead-letters) for surfacing a breaking change.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] Pure `StreamSchema::diff(discovered) -> SchemaDiff { additive: [Field], breaking: [reason] }`: new field
  = additive; shared field with a changed type = breaking. (Dropped/nullability shifts intentionally ignored —
  inference reads them from a sample, too noisy to gate a run on; a concrete type change is the hard break.)
- [x] **Additive**: `Store::evolve_schema` merges the new fields into the stored schema + clears the flag; the
  run proceeds.
- [x] **Breaking**: the `stream_schemas.broken` reason is set ("field `n` type changed …"); enforcement already
  dead-letters the drifted records (blocked from the dest), so it can't silently corrupt.
- [x] Escape hatch: `App::accept_schema` forgets the stored schema → the next run re-baselines the new shape.
- [x] `diff` unit test (additive + breaking) + integration test: boolean→integer drift flags `broken` AND
  dead-letters all 3. Workspace + clippy clean.

## Status Updates

### 2026-07-07 — done (`e51c85d`)

`StreamSchema::diff` → `SchemaDiff{additive, breaking}` (new field additive, type change breaking). `Store::
evolve_schema` runs post-sync: capture if none, merge additive + clear flag, or set `broken` on a type change.
Key fix: the run samples the **post-mapping / pre-enforce** shape, so a breaking drift is detected even though
enforcement then dead-letters those records (proven — the test asserts both `broken` set AND 3 dead-lettered).
`App::schema_broken` surfaces the flag; `App::accept_schema` (delete → re-baseline) is the operator escape
hatch. Workspace + clippy clean. **Complete.** Next: [[WEIR-T-0121]] surfaces schema + drift in the UI.
