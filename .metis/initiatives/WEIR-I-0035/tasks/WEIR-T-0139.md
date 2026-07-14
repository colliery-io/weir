---
id: f1-3-resident-supervision-in-the
level: task
title: "F1.3 — Resident supervision in the worker (perpetual lease, requeue-on-exit, skip scheduler)"
short_code: "WEIR-T-0139"
created_at: 2026-07-09T15:34:41.878010+00:00
updated_at: 2026-07-09T17:04:31.742594+00:00
parent: WEIR-I-0035
blocked_by: [WEIR-T-0137, WEIR-T-0138]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.3 — Resident supervision in the worker (perpetual lease, requeue-on-exit, skip scheduler)

## Parent Initiative

[[WEIR-I-0035]] (F1 — Long-lived source runtime)

## Objective

Make the orchestrator run a resident source as a **never-completing work unit**, supervised by the existing
lease/heartbeat/backoff machinery: enqueue-once, hold the lease with a perpetual heartbeat, run the F1.2 resident
loop, and on exit/error **requeue with exponential backoff** so it restarts and resumes from checkpoint. No bespoke
supervisor — reuse the work queue.

## Acceptance Criteria

## Acceptance Criteria

- [ ] `Worker::tick` branches on `ExecutionMode`: `RunOnce` → today's path; `Resident` → run the F1.2 resident loop.
- [ ] A resident unit is **enqueued once** and stays `leased` with a heartbeat that never stops while it runs; a
  lightweight `resident` marker distinguishes it (observability) from a stuck lease.
- [ ] On resident loop exit/error the unit **requeues with backoff** (`base_delay * (1 << attempt)`) and re-claims;
  execution resumes from the last committed checkpoint (at-least-once, [[WEIR-A-0011]]).
- [ ] The **scheduler skips resident mode** (no interval/cron firing); a stopped resident source is not re-fired.
- [ ] Runner death → lease expiry → **any runner in the tenant fleet re-claims** the resident unit (no pinning).
- [ ] A manual **stop** transitions the unit out of resident and cancels the loop (via F1.2 stop signal).

## Implementation Notes

### Technical Approach
- Worker branch + retry: `weir-orchestrator/src/lib.rs:Worker::tick` (~:940, requeue ~:1047).
- Heartbeat / lease: `Relay::heartbeat` (~:661), `Relay::claim` (~:440 tenant filter), `Relay::requeue`,
  `Relay::has_active` (~:761).
- Scheduler skip: `Scheduler::tick` (~:1266).
- Executor dispatch: `WorkExecutor::execute` / `InProcessExecutor::execute` (~:803).

### Dependencies
[[WEIR-T-0137]] (mode), [[WEIR-T-0138]] (resident loop + stop token).

### Risk Considerations
- Perpetual heartbeat must not mask a genuinely-hung source — pair with the `resident` marker + liveness so health
  ([[WEIR-I-0024]]) can tell "alive & leased forever" from "stuck". (F3's liveness heartbeat sharpens this later.)
- Ensure requeue-on-exit can't hot-loop a permanently-failing source into a tight restart cycle — respect backoff.

## Status Updates

**2026-07-09 — COMPLETED (supervision mechanics, build-verified green).** `crates/weir-orchestrator/src/lib.rs`
+ `tests/scheduler.rs`.
- `InProcessExecutor` gained a `stops: Arc<Mutex<HashMap<i64, StopHandle>>>` registry + `stop(id)`/`stop_all()`;
  `execute()` branches on `spec.execution_mode.is_resident()` → `stop_channel()` + `Engine::run_resident(..., token)`
  + register/deregister (drop-to-cancel). Run-once unchanged.
- **Perpetual lease** falls out for free: the existing heartbeat task extends the lease for the whole life of
  `execute()`, which for a resident unit doesn't return until stop/error. New `ExecutionMode::is_resident()` /
  `restart_backoff_ms()` helpers.
- **Requeue-on-exit**: resident `Failed`/exit **always requeues** with exponential backoff (ignoring `max_attempts`
  → unbounded supervised restart, resume from last checkpoint); clean stop → `Completed`/`mark_done`. Run-once retry
  unchanged.
- **Scheduler skips resident specs** (`Scheduler::tick`) — no cron/interval re-fire; can't resurrect a stopped
  resident or double-start a running one. Runner-death re-claim (no pinning) unaffected.
- **Verify:** fmt/clippy clean; `cargo build --workspace` green; orchestrator lib 5 + scheduler 6 pass (incl. new
  `skips_resident_specs`). PG-gated integration tests not run (no docker).

**DEFERRED to F1.5 ([[WEIR-T-0141]]) — control-plane wiring (prerequisites for the resident e2e test):**
1. **Enqueue-once START path** — since the scheduler skips resident specs, a resident source's one-time
   `Relay::plan(...)` must come from an explicit start in `weir-app`/CLI. Not wired here → a resident connection
   won't launch until this lands. (Acceptance "#enqueued once" mechanism present; trigger deferred.)
2. **Manual-STOP control path** (acceptance #6) — `stop`/`stop_all` fire the `StopHandle`, but the API→running-worker
   plumbing isn't wired end-to-end.
Both are re-homed into F1.5 because its resident e2e/restart test cannot run without a start (and a stop) path.