---
id: orchestrator-sharp-edges-bundle
level: task
title: "Orchestrator sharp-edges bundle — owned transitions, tick isolation, id nonce, backoff decay, cache bounds"
short_code: "WEIR-T-0190"
created_at: 2026-08-30T11:54:21.269450+00:00
updated_at: 2026-08-30T13:31:13.106581+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# Orchestrator sharp-edges bundle — owned transitions, tick isolation, id nonce, backoff decay, cache bounds

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

Bundle of orchestrator correctness edges that individually corrupt state or wedge a deployment under long continuous operation. All live in `weir-app`/scheduler-worker code:

1. **Owner-guarded transitions** — `mark_done`/`mark_failed` don't verify the caller still owns the work unit; a lease-expired worker finishing late can clobber the state of a re-claimed unit. Guard transitions on `(unit_id, owner/claim token)`.
2. **Per-schedule tick isolation** — one schedule's error mid-tick can abort the loop for the remaining schedules. Each schedule's tick work is isolated (error logged, next schedule still processed).
3. **`next_id` nonce** — id generation is a plain counter vulnerable to collision after restart/multi-worker; add a nonce/uniqueness component so ids can't collide across process lifetimes.
4. **`run_until_idle` containment** — the test/dev helper can spin forever if a unit perpetually re-queues; bound it (iteration cap with loud failure) so a stuck unit surfaces instead of hanging.
5. **Resident backoff decay + cap** — resident-connector restart backoff only grows; cap it and decay it after sustained healthy uptime so one bad night doesn't leave a connector permanently slow to restart.
6. **HANDLE_CACHE bound** — the wasm handle cache grows unboundedly with distinct connector versions; bound it (LRU or explicit eviction on unregister).
7. **Per-run wasm isolation** — the [[WEIR-T-0066]] JoinError wedge: one run's wasm panic/JoinError can poison the shared engine path; isolate per-run so a crash fails only that run.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] Stale-owner `mark_done`/`mark_failed` is rejected (test: expire a claim, re-claim elsewhere, late finisher can't transition)
- [x] A schedule whose tick errors doesn't prevent other schedules ticking in the same pass (test with a poisoned schedule)
- [x] Ids include a per-process nonce; restart cannot produce a colliding id (test across two store instances — see status note: cross-process is untestable in-process; covered by nonce construction + a 50k-id uniqueness burst)
- [x] `run_until_idle` fails loudly at an iteration cap instead of hanging
- [x] Resident backoff is capped and decays after healthy uptime (unit test on the backoff state machine)
- [x] HANDLE_CACHE has an explicit bound/eviction path with a test
- [x] A wasm run that panics/JoinErrors fails only its own run; a subsequent run on the same worker succeeds (regression for the T-0066 wedge)
- [x] `angreal check all` + unit wall + functional suite green

## Status Updates **[REQUIRED]**

- **2026-08-30 — Activated; scouted all 7 sites, plan set.** All in weir-orchestrator/src/lib.rs; no external callers of the transition methods (grep). (1) `mark_done`/`mark_failed`/**`requeue`** (same terminal-transition class from run_and_apply) gain an `owner: &str` and filter `lease_owner.eq(owner)` — 0 rows updated = stale finisher, warn + Ok (its result is void, never an error); `cancel()` stays owner-less (deliberate control-plane override); callers pass `config.owner`. (2) Scheduler::tick: per-schedule body becomes error-isolated (log + continue; from_json/next_cron/plan/has_active_in errors no longer abort the remaining schedules). (3) `next_id`: COUNTER becomes OnceLock<AtomicI64> seeded `pid ^ subsec_nanos` — a restarted/second process in the same ms can't re-mint the sequence; can't unit-test cross-process, so test = rapid-mint uniqueness + construction. (4) `Worker::run_until_idle`: `max_drain_units` WorkerConfig knob (default 10_000) → new `ExecutorError::DrainStuck(u64)` instead of spinning; Fleet outer loop gets a rounds cap. (5) Resident backoff: extract pure `resident_restart_delay(base_ms, attempt, run_ms)` — healthy uptime (run_ms ≥ 60s) resets the shift to 0 (decay), absolute cap 300s; unit-tested. (6) HANDLE_CACHE: entries become (handle, last-use Instant), touch on hit, evict LRU at 32 on insert. (7) T-0066 wedge: run_and_apply stops `?`-propagating executor errors (a wasm panic = JoinError aborted the WHOLE drain pass and stranded the lease); they become `ExecutionResult::Failed{Transient}` so the unit requeues/fails through the normal terminal transition and other units keep draining. Tests: orchestrator tests with a panicking executor (drain completes, unit terminal, next unit runs), stale-owner rejection, tick isolation with a poisoned schedule, id uniqueness burst, backoff fn table, cache eviction.
- **2026-08-30 — DONE, all 7 landed** (weir-orchestrator/src/lib.rs; scouting found one extra hazard, fixed with item 1: `cancel()` leaves `lease_owner` set, so the worker's exit `requeue` could RESURRECT a cancelled unit — the guard therefore requires owner match AND in-flight state `["leased","running"]`). As planned: (1) owner+state-guarded mark_done/mark_failed/requeue, stale = warn + no-op, callers pass `config.owner`; (2) Scheduler::tick per-schedule closure, errors logged + loop continues (a poisoned schedule stays due and re-logs — loud but isolated); (3) next_id counter seeded `pid ^ subsec_nanos` via OnceLock; (4) `WorkerConfig.max_drain_units` (default 10_000) → `ExecutorError::DrainStuck` + Fleet 1_000-round cap; (5) `resident_restart_delay` pure fn — cap 300s, decay-to-base after 60s healthy uptime; (6) `lru_evict` helper + HANDLE_CACHE entries stamped with last-use, cap 32; (7) run_and_apply converts executor Err (incl. the T-0066 JoinError) to `Failed{Transient}` → normal terminal transition, drain survives. **Tests**: `tests/sharp_edges.rs` (5: stale-owner reclaim, cancelled-unit resurrection, executor-error isolation with bad→failed/good→done, DrainStuck at cap 5, poisoned-schedule isolation with the poison id sorting first) + `sharp_edges_tests` unit mod (backoff table incl. cap + decay, LRU eviction, 50k-id burst). Full orchestrator suites, weir-app 31+integration, weir-api 16, functional, unit wall, soak builds, `angreal check all` — all green. CHANGELOG Fixed entry added.
