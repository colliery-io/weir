---
id: clean-shutdown-stranded-resident
level: task
title: "Clean shutdown + stranded-resident recovery (stop_all on ctrl-c, reclaim at startup)"
short_code: "WEIR-T-0170"
created_at: 2026-08-16T15:24:08.729841+00:00
updated_at: 2026-08-25T08:13:59.471432+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


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

- [x] serve/api/runner shutdown paths stop residents: `App::serve` and `App::run_workers` call `relay.stop_all_residents()` after their shutdown-select loops break (covers `weir serve`, `weir api`'s embedded serve, and `weir runner`), so the blocking runs end at the next poll boundary and runtime teardown drains
- [x] Expired-lease reclaim runs independent of due-pending depth: `Fleet::run_until_idle` sweeps `reclaim_expired()` FIRST, before the active-tenants early-return — serve's per-poll fleet drive is the periodic sweep; the pod-per-tenant runner path already swept per-poll via `Worker::run_until_idle`
- [x] `tests/resident_real.rs` completes without wedging (both tests, ~6s warm); new `scheduler.rs::fleet_reclaims_stranded_resident_with_no_due_work` proves ghost-claim → lease expiry → fleet sweep → re-run with nothing else queued
- [x] The self-Arc cycle is documented as resolved by the shutdown layer: the registry (now on the shared `Relay`) is still transitively reachable from the run's own relay clone — inherent, since the run needs the relay — so drop-to-cancel alone can never end a live resident; the executor struct comment states that shutdown paths MUST call `stop_all_residents`, and the daemons + resident_real do

## Implementation Notes

Known limitation to respect (documented follow-on in `wasm_resident_ws_engine.rs`): a guest parked in a synchronous socket read only honors stop between frames — cooperative stop may need a bounded wait + detach rather than a join. The two unit "resident does not wedge" tests use fake executors whose futures drop cleanly, so they cannot catch this class — the real-path proof is resident_real. Related known F1 deferral lives in WEIR-I-0037 (resident_capable spec fields); don't expand scope into it.

## Status Updates **[REQUIRED]**

**2026-08-25 — implemented + verified (ralph run).**

- **Stop registry moved to the shared `Relay`** (`resident_stops` + `register/deregister/stop_resident/stop_all_residents`): one shutdown call reaches every live resident across every per-tenant executor the fleet spawns. `InProcessExecutor` slims to `{store, relay}`; its `stop`/`stop_all` (incl. the heartbeat lease-loss path's trait `stop`) delegate to the relay, preserving all existing semantics.
- **Daemon shutdown wiring**: `App::serve` + `App::run_workers` fire `stop_all_residents()` after their loops break, with an info log of the count.
- **Fleet-level reclaim sweep**: `Fleet::run_until_idle` runs `reclaim_expired()` before the active-tenants check — the stranded case (leased-to-a-ghost resident, nothing else due) now recovers on the next serve poll without operator stop-then-start; `App::start`'s no-op resolves itself once the unit is back to pending.
- **Tests**: `resident_real` extended to fire the shutdown stop (asserting exactly 1 live resident stopped) — both tests pass without wedging; new `fleet_reclaims_stranded_resident_with_no_due_work` in scheduler.rs. Full verification: orchestrator suites 26/26 (backfill 1, heartbeat 3, orchestrator 8, resident_real 2, scheduler 12), weir-app cli 8/8 + serve 2/2, `angreal test unit` all green, `angreal check all` clean.
- **Diagnosis note for the record**: the "still wedges after the fix" scare during this task was NOT the wedge — it was the resident fixture crate cold-compiling *inside* the test (wit-bindgen guest deps) under heavy Docker contention, repeatedly reset by SIGKILLed runs. On a quiet machine the test passes in ~6s warm / ~56s cold. The original no-stop wedge was real and is fixed by the shutdown layer.
- Ctrl-c bound: shutdown latency is bounded by the resident's poll boundary (cadence); a guest parked in a synchronous socket read still only honors stop between frames (pre-existing, documented in `wasm_resident_ws_engine.rs`; out of scope per the notes).

**2026-08-28 — post-review hardening (pre-v0.0.1 release review).** The adversarial release review flagged a cross-talk hazard introduced by sharing the registry on the Relay: entries were keyed by `work_unit_id` alone, so a STALE run (lease expired mid-run, unit re-claimed in the same process) returning late would `deregister` — and thereby drop-cancel — the REPLACEMENT run's StopHandle; `run_resident` returns Ok on a clean stop, so the healthy resident would be marked done and silently never restarted. Pre-registry this was impossible (per-executor maps). Fixed with generation-guarded deregistration: `register_resident_stop` now returns a monotonically increasing generation (stored alongside the handle; overwrite still drop-cancels the stale run, which is correct), and `deregister_resident_stop(id, generation)` removes only on a generation match. New test `stale_resident_deregister_cannot_drop_replacement_handle` (orchestrator.rs); suite 10/10.
