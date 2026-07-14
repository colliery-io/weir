---
id: f1-follow-up-behavioral-cadence
level: task
title: "F1 follow-up: behavioral cadence/event-arrival e2e via the unified resident fixture"
short_code: "WEIR-T-0151"
created_at: 2026-07-14T02:24:30.791356+00:00
updated_at: 2026-07-14T02:37:53.861482+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1 follow-up: behavioral cadence/event-arrival e2e

## Parent Initiative

[[WEIR-I-0035]] (F1). Followup identified during development — confirm/strengthen behavioral coverage of the
resident modes, reusing the unified `wasm-fixtures/resident` fixture (no new fixture crates).

## Objective

Audit the behavioral e2e coverage of poll-cadence / event-arrival / ws, and close the one genuine gap.

## Coverage audit (what was already proven)

- **poll cadence** — `weir-engine/tests/wasm_resident_engine::resident_poll_reads_bounded_batch_per_cadence_cycle`:
  bounded batch per cadence cycle (`rows == polls * rows_per_poll`), cadence-bounded poll count (not an unbounded
  stream). ✅
- **poll through the real Fleet/InProcessExecutor** — `weir-orchestrator/tests/resident_real::
  resident_does_not_block_runonce_real_executor`. ✅
- **event-arrival (tail)** — `wasm_resident_engine::resident_tail_emits_per_arrival`: K arrivals → K rows, 0 → 0. ✅
- **ws arrival + supervised reconnect** — `weir-engine/tests/wasm_resident_ws_engine` (3 tests). ✅

**Gap:** tail/event-arrival was only proven at the **engine** level; it was never driven through the real
`Fleet`/`InProcessExecutor` worker path.

## What was added

`weir-orchestrator/tests/resident_real.rs::tail_arrivals_drain_through_real_fleet` — a `mode:tail` unit (finite
config-arrival burst) drained through the real `Fleet` + `InProcessExecutor` + wasm to `done`, proving the
arrival-driven path flows through the worker/executor. The exact K→K count stays engine-proven (there's no Relay
rows accessor in an orchestrator test; asserting `done` proves the drain path).

## Acceptance Criteria

- [x] Coverage audited + stated per mode.
- [x] Tail arrival proven through the **real** Fleet/InProcessExecutor (`tail_arrivals_drain_through_real_fleet`).
- [x] No new fixture crates (reused the unified `resident` fixture).
- [x] All existing tests stay green.

## Status Updates

**2026-07-14 — COMPLETE.** Audit above; added `tail_arrivals_drain_through_real_fleet`. Poll/ws already had
real-path or reconnect coverage; only tail-through-Fleet was missing. Not committed.
