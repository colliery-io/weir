---
id: orchestrator-work-unit-store
level: task
title: "orchestrator work-unit store — WorkSpecRow/RunRow + store module"
short_code: "WEIR-T-0134"
created_at: 2026-07-08T17:27:37.206823+00:00
updated_at: 2026-07-08T23:53:23.821713+00:00
parent: WEIR-I-0030
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0030
---

# orchestrator work-unit store — WorkSpecRow/RunRow + store module

## Parent Initiative

[[WEIR-I-0030]] — the second of two: the same deepening for `work_units` in the orchestrator, reusing the
[[WEIR-T-0133]] pattern.

## Objective

Replace the positional `work_units` row-tuples in `weir-orchestrator` with `WorkSpecRow` / `RunRow` (diesel derives)
and a `store` module. Delete `SpecTuple`, `RunTuple`, and the inline full-row `.select`/`.values`/`.load::<(…)>`
plumbing. Leave genuinely-partial projections (e.g. the claim's `(id, attempt)`) as small typed tuples — they're not
the scatter.

## Reference

- `crates/weir-orchestrator/src/lib.rs` — `SpecTuple` (:333), `RunTuple` (:358), `plan` insert `.values` (:411),
  `load` select+rebuild (:611–641), run-feed selects (~:795, :837, :1326, :1371), `from_json` (:878).
- `crates/weir-schema/schema/generated/schema.rs` — `work_units`.
- `crates/weir-schema/tests/dual_backend.rs` — `work_units_claim_round_trips` (:191) for the both-backends proof.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `WorkSpecRow` (Queryable/Selectable, the spec subset) + `RunFeedRow` (the feed subset) in a new
  `store.rs`; `into_work_spec`/`into_run_row` own the JSON decode + the derived `duration_ms` — **once**.
- [x] `weir-orchestrator::store` exposes `insert_pending` (plan), `load_spec` (load), `run_feed` (recent); the three
  `plan*` entry points and `load`/`recent` call it. (`Insertable` not needed — `plan` sets control columns id/state,
  so the insert is one `.values` centralized in `store::insert_pending`.)
- [x] `SpecTuple` and `RunTuple` **deleted**; `lib.rs` −112. Partial projections left as-is: the claim candidate
  `(id, attempt)`, the claim/lease guarded `UPDATE`, and `active_tenants`' single-column select.
- [x] `dual_backend.rs::work_unit_spec_row_round_trips` proves the `WorkSpec` subset selects via `as_select` on
  **both** SQLite and live Postgres.
- [x] Full workspace + clippy `-D warnings` + the orchestrator suite (lib/backfill/heartbeat/orchestrator/scheduler)
  green; needed `diesel-dualdb` added to weir-orchestrator's deps (for `DualConnection`).

## Implementation Notes

### Technical Approach
Mirror [[WEIR-T-0133]]. The claim/lease guarded `UPDATE … WHERE state='pending'` stays hand-written — it's a
conditional write, not a row mapping. Only the full-row load/insert/run-feed collapse into `WorkSpecRow`/`RunRow`;
the JSON columns (`stream, source_ref, dest_ref, source_config, dest_config, seed_cursor, partition`) move their
`from_json`/`to_json` into `From`/`TryFrom`.

### Dependencies
[[WEIR-T-0133]] — reuses the row pattern and confirms whether `as_select()` composes (else the shared
`SELECT_COLUMNS` fallback).

### Risk Considerations
The orchestrator has more inline tuple sites than the app, and some are **partial projections**, not full rows —
don't force those into row structs. Distinguish "full-row select/insert" (collapse) from "ad-hoc projection" (leave).

## Status Updates

### 2026-07-08 — done

Same shape as [[WEIR-T-0133]], smaller footprint: `SpecTuple` was consumed only by `load`, `RunTuple` only by
`recent`, and the three `plan*` variants all delegate to one `plan` insert — so `store.rs` needed just
`WorkSpecRow` (spec subset), `RunFeedRow` (feed subset), and `insert_pending`. The claim candidate `(id, attempt)`,
the guarded lease `UPDATE`, and `active_tenants` stay as-is — partial projections / conditional writes, not row
mappings. `as_select` composes over `DualConnection` here too (proven on SQLite + live Postgres). `lib.rs` −112.
No `cargo fmt` was run (the stale-toolchain drift from T-0133); edits hand-formatted to match. Workspace + clippy
`-D warnings` + orchestrator suite green.

Both tasks of [[WEIR-I-0030]] complete — the row↔struct scatter is gone from both the app and the orchestrator.
