---
id: f1-follow-up-cross-process-mid
level: task
title: "F1 follow-up: cross-process mid-stream stop (lease-loss cancels a remote resident run)"
short_code: "WEIR-T-0149"
created_at: 2026-07-14T02:24:18.569439+00:00
updated_at: 2026-07-14T02:25:11.107877+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1 follow-up: cross-process mid-stream stop

## Parent Initiative

[[WEIR-I-0035]] (F1). Closes the F1.5/F1.7 "cross-process mid-stream stop" gap.

## Objective

Make a durable `App::stop` (→ `Relay::cancel`) halt the **actual** running resident, not just mark the DB unit
`done` — even when the resident is executing on a *different* runner than the one that received the stop. Before
this, cancel was durable but the in-flight run kept going until it happened to hit a checkpoint/lease-expiry, because
only the same-process `InProcessExecutor::stop` (`StopHandle`) could interrupt it.

## Type / Priority

Bug (resident supervision correctness) · P2.

## Acceptance Criteria

- [x] Cancelling a resident from anywhere stops the actual run within ~one heartbeat interval (no direct
  `StopHandle` call needed).
- [x] Clean stop (unit already terminal) — no spurious requeue.
- [x] Run-once + existing resident tests unaffected.

## Implementation

`WorkExecutor` gained `fn stop(&self, work_unit_id: i64) {}` (default no-op); `InProcessExecutor` implements it by
firing the run's `StopHandle`. The **heartbeat task** in `Worker::run_and_apply` (`crates/weir-orchestrator/src/lib.rs`)
already breaks when `Relay::heartbeat` returns `false` (lease lost — exactly what `Relay::cancel` from any process
causes); it now **calls `executor.stop(id)` on that lease-loss** before breaking, firing the engine stop token so
`Engine::run_resident` exits cleanly. So the heartbeat is the cross-process observer: cancel → next beat sees the
lost lease → stop the real run.

## Status Updates

**2026-07-14 — DONE.** Wired trait `stop` + `InProcessExecutor` impl + heartbeat lease-loss→stop. New test
`crates/weir-orchestrator/tests/scheduler.rs::resident_cancel_stops_the_running_task_cross_process`: starts a
resident whose run loops until its stop token fires, detaches it, then `Relay::cancel` (NO direct `StopHandle`) →
the heartbeat fires `stop` → the run ends. **Passes** (0.07s, deterministic mock — no wasm/docker). fmt/clippy clean;
`cargo build --workspace` green; scheduler 10 / orch lib 8 / weir-app lib 25 — no regressions. Not committed.
