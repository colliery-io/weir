---
id: f1-follow-up-per-connection
level: task
title: "F1 follow-up: per-connection resident cadence plumbing (declared cadence → ExecutionMode)"
short_code: "WEIR-T-0150"
created_at: 2026-07-14T02:24:20.889610+00:00
updated_at: 2026-07-14T02:25:15.894289+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1 follow-up: per-connection resident cadence plumbing

## Parent Initiative

[[WEIR-I-0035]] (F1).

## Objective

A resident connection should poll at its **declared** cadence, not a hardcoded default — `every_secs` on the
connection threads through to `ExecutionMode::Resident.cadence_ms`, clamped to the ~20ms floor.

## Acceptance Criteria

- [x] A resident connection with `every_secs = X` yields `ExecutionMode::Resident { cadence_ms: Some(X*1000) }`
  through `work_spec`.
- [x] Below-floor cadence clamps to 20ms (faster → event triggers, not polling).

## Status Updates

**2026-07-14 — DONE (the plumbing already landed in F1.9; this task adds the test + confirmation).** The stale
follow-on note said `parse_execution_mode` "uses defaults" — that was true after F1.4/F1.6 but F1.9 already fixed
it: `parse_execution_mode(s, every_secs)` (`crates/weir-app/src/lib.rs:1834`) builds
`Resident { cadence_ms: every_secs.map(|s| ((s*1000.0) as u64).max(20)), .. }`, and `work_spec` passes
`c.every_secs` (line ~1885). CLI (`connection add --execution-mode resident --every N`) + API DTO already round-trip
it. Added a regression test `weir-app::catalog_tests::resident_cadence_from_every_secs_with_20ms_floor` (every_secs
2.0 → cadence_ms 2000; 0.001 → clamped 20). **Passes**; weir-app lib 25, no regressions. Not committed.
