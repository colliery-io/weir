---
id: clean-shutdown-stranded-resident
level: task
title: "Clean shutdown + stranded-resident recovery (stop_all on ctrl-c, reclaim at startup)"
short_code: "WEIR-T-0170"
created_at: 2026-08-16T15:24:08.729841+00:00
updated_at: 2026-08-16T15:24:08.729841+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Clean shutdown + stranded-resident recovery (stop_all on ctrl-c, reclaim at startup)

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Two shutdown/recovery holes in the resident path. (1) Ctrl-c with a live resident hangs the process: `InProcessExecutor::stop_all` has zero callers, and the resident's spawn_blocking closure holds an Arc to the stops map containing its own StopHandle, so drop-to-cancel never fires — the resident_real test wedges at tokio runtime teardown (reproduced twice during the review). (2) After a crash, a leased-but-expired resident is never reclaimed when no other due pending work exists anywhere, and `App::start` no-ops because has_active still sees the stale lease — only stop-then-start recovers it. Wire clean shutdown and lease reclaim that doesn't depend on pending work.

## Evidence (2026-08-16 alpha review)

- `crates/weir-orchestrator/src/lib.rs:1053-1058` — `stop_all` defined, zero production callers (verified by grep).
- `crates/weir-cli/src/main.rs:377-411` — ctrl-c just breaks the serve/runner loop.
- `crates/weir-orchestrator/src/lib.rs:1066, 1090-1105` — the StopHandle self-Arc mechanism keeping the blocking task alive.
- `crates/weir-orchestrator/src/lib.rs:1483, 1545-1549` — reclaim_expired only runs inside Worker::run_until_idle; Fleet::run_until_idle returns early when active_tenants (due-pending only) is empty.
- `crates/weir-app/src/lib.rs:383` — start() returns Ok(None) on the stale leased unit.
- `crates/weir-orchestrator/tests/resident_real.rs` — deterministically reproduces the teardown wedge.

## Acceptance Criteria **[REQUIRED]**

- [ ] serve/api/runner shutdown paths cooperatively stop residents (stop_all or equivalent) before exit; a process with a live resident exits cleanly on ctrl-c
- [ ] Expired-lease reclaim runs at startup and periodically, independent of due-pending depth, so a crashed resident recovers without operator stop-then-start
- [ ] `tests/resident_real.rs` completes without wedging; new test: kill runner → restart → resident reclaimed with no other work queued
- [ ] The StopHandle self-Arc cycle is broken, or explicitly documented as resolved by the shutdown-layer fix (with the reasoning)

## Implementation Notes

Known limitation to respect (documented follow-on in `wasm_resident_ws_engine.rs`): a guest parked in a synchronous socket read only honors stop between frames — cooperative stop may need a bounded wait + detach rather than a join. The two unit "resident does not wedge" tests use fake executors whose futures drop cleanly, so they cannot catch this class — the real-path proof is resident_real. Related known F1 deferral lives in WEIR-I-0037 (resident_capable spec fields); don't expand scope into it.

## Status Updates **[REQUIRED]**

*To be added during implementation*
