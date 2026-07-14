---
id: f1-2-resident-engine-loop-share
level: task
title: "F1.2 — Resident engine loop (share drive loop; no-teardown + cooperative stop)"
short_code: "WEIR-T-0138"
created_at: 2026-07-09T15:34:36.745048+00:00
updated_at: 2026-07-09T16:52:18.892102+00:00
parent: WEIR-I-0035
blocked_by: [WEIR-T-0137]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.2 — Resident engine loop (share drive loop; no-teardown + cooperative stop)

## Parent Initiative

[[WEIR-I-0035]] (F1 — Long-lived source runtime)

## Objective

Give the engine a **resident execution path**: refactor `Engine::sync_with` so the streaming drive loop is shared,
then add a resident entry that holds the connector handle open, drains the `read` stream indefinitely, commits
transactionally on source-driven `ReadMessage::Checkpoint`, and exits only on a **cooperative stop signal** or a
stream error. Run-once behavior is unchanged.

## Acceptance Criteria

## Acceptance Criteria

- [ ] The `ReadMessage` drive+map+write+checkpoint loop is factored so run-once and resident share it.
- [ ] A resident run holds one configured connector instance open across the whole run (no per-cycle reload).
- [ ] Commits happen on each source `Checkpoint`, mid-stream (not only at end-of-stream).
- [ ] A cooperative **stop** cancels the stream promptly and shuts down cleanly (drop-to-cancel per [[WEIR-A-0029]]).
- [ ] A stream error/`ConnectorError` ends the run with a `Transient` outcome (so F1.3 can requeue) and leaves the
  last committed checkpoint intact.
- [ ] Existing `weir-engine` run-once tests unchanged and green.

## Implementation Notes

### Technical Approach
- Loop + checkpoint commit: `weir-engine/src/lib.rs:Engine::sync_with` (~:694, commit ~:759).
- Handle lifecycle: `weir-runtime/src/lib.rs:ConnectorHandle::read` (~:150); handle cache `HANDLE_CACHE` in
  `weir-orchestrator` (keep warm).
- Add a resident entry (e.g. `Engine::run_resident`) sharing the extracted loop; take a stop token
  (e.g. cancellation) so the worker (F1.3) can stop/rebalance it.

### Dependencies
[[WEIR-T-0137]] (needs `ExecutionMode` to select the resident path).

### Risk Considerations
- `sync_with` uses `futures::executor::block_on`; keep the resident loop's cancellation cooperative so a stop
  doesn't wedge on a blocked poll.
- Don't regress run-once checkpoint/transaction semantics ([[WEIR-A-0011]]) while extracting the shared loop.

## Status Updates

**2026-07-09 — COMPLETED (build-verified green).** Single-file change to `crates/weir-engine/src/lib.rs`;
`weir-orchestrator` untouched (dependency direction preserved — engine does NOT import `ExecutionMode`).
- `sync_with`/`sync` now delegate to a private shared `Engine::drive(..., stop: Option<StopToken>)` (old body +
  stop-aware loop); run-once public signatures unchanged.
- New `pub fn Engine::run_resident(..., stop: StopToken)` = `drive(Some(stop))`; holds one connector instance open,
  drains indefinitely, commits transactionally on each source `Checkpoint` (same tx path as run-once).
- Stop = in-crate `oneshot` via `pub fn stop_channel() -> (StopHandle, StopToken)`; `StopHandle::stop()` **and drop**
  both cancel (drop-to-cancel). The loop `select`s each `read_stream.next()` poll against the stop rx → honored at
  the next poll boundary, clean shutdown, last checkpoint intact.
- New `EngineError::ResidentStreamEnded` — a resident stream hitting end-of-stream is abnormal → `Err`, so F1.3
  requeues.
- **Verify:** fmt clean; `clippy -p weir-engine` clean; `cargo build --workspace` green; `weir-engine` lib 14 passed
  + integration tests pass (postgres/s3/throughput ignored — pre-existing, need docker). No run-once regressions.
- **For F1.3:** branch on `ExecutionMode` → call `run_resident` vs `sync_with`; own the `stop_channel()`/`StopHandle`
  (stop on shutdown/rebalance); map `run_resident` `Err` (incl. `ResidentStreamEnded`) → transient requeue + backoff,
  resume from last checkpoint.