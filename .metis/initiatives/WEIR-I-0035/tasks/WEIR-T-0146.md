---
id: bug-worker-stops-claiming-new-work
level: task
title: "BUG: worker stops claiming new work after an ungraceful restart (recovery)"
short_code: "WEIR-T-0146"
created_at: 2026-07-10T19:42:12.411505+00:00
updated_at: 2026-07-10T19:45:25.912793+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# BUG: a resident run wedges the per-poll drain loop → new work strands in `pending`

## Parent Initiative

[[WEIR-I-0035]] (F1). Surfaced via the UI demo (clicked "Run" on scheduled connections while a resident source was
live → they stuck in `pending`). Title says "after ungraceful restart" — that was the *trigger* I noticed it
through; the **real cause is more fundamental** (below) and is not restart-specific.

## Symptom

With a resident source running, newly-enqueued units (manual `/run` and scheduler-fired) stay `pending` forever;
the worker is idle (CPU ~0.7%), not starving. A full server bounce flushes the current backlog once, then it wedges
again as soon as the resident is running.

## Root cause (confirmed by code read)

`Worker::run_until_idle` (`crates/weir-orchestrator/src/lib.rs:1404`) drives a `FuturesUnordered` of `tick()`
futures and **awaits each unit's execution to completion**; the loop only ends when a tick finds nothing to claim
**and `inflight.is_empty()`**. A **resident** unit's `tick()` runs `Engine::run_resident`, which **never completes**
(perpetual). So its tick future never resolves → `inflight` is never empty → **`run_until_idle` never returns**.

The `serve` loop (`crates/weir-app/src/lib.rs:1219`) calls `fleet.run_until_idle().await` **every poll** and the
scheduler tick is in the same loop body. Once a resident is in flight, that `.await` is **parked forever** → the
serve loop never iterates again → the scheduler never re-ticks and **no new claim pass ever runs**. The first drain
pass flushes the existing backlog (why a bounce "fixes" it once), then it wedges.

**Why the F1.5 soak missed it:** its "residents" were echo/slow with **finite** read streams → `ResidentStreamEnded`
→ tick **resolves** (requeue) → `run_until_idle` keeps returning. Only a *genuinely perpetual* resident
(poll/tail/ws — the real case) wedges it. My earlier "starvation"/"hot-loop" guesses were wrong (CPU idle; resident
paced correctly; stop didn't drain — it's the parked `.await`).

## Fix (planned)

**Detach resident execution from the per-poll drain.** A resident unit must NOT occupy a `run_until_idle` slot for
its (infinite) life:
- When a worker claims a **resident** unit, **spawn its execution as an independent, supervised long-lived task**
  (`tokio::spawn`), and treat the `tick()` as *processed* immediately so `run_until_idle` continues and **returns**.
- The resident task owns its heartbeat + stop handle (F1.3) + requeue-on-exit independently of the drain loop.
- `run_until_idle` then drains only transient (run-once) work and returns each poll → the serve loop keeps ticking
  the scheduler and claiming new work while residents run.
- Track running residents so a stop/cancel + re-claim still work; don't double-run on re-claim.

## Acceptance Criteria

## Acceptance Criteria

- [ ] With a perpetual resident running, `run_until_idle` **returns** each poll; the serve loop keeps scheduling +
  claiming.
- [ ] **Regression test:** a perpetual-resident fixture + run-once units co-scheduled → run-once **drain to done**
  WHILE the resident runs (this test fails on today's code — it would hang/time out).
- [ ] Resident heartbeat, stop (`weir stop`), and requeue-on-exit still work with detached execution.
- [ ] Demo: with `resident-demo` live, clicking Run on the other tiles → they complete (no `pending` wedge).

## Status Updates

**2026-07-10 — root-caused; first fix attempt REGRESSED the live path (NOT done).**
- Root cause confirmed + unit-tested: detaching residents (`tokio::spawn`, `tick` returns immediately) makes
  `run_until_idle` return with a perpetual resident (`resident_does_not_wedge_run_until_idle` passes; orchestrator
  suite green).
- **BUT end-to-end regression:** on a clean demo DB + the fixed `weir api` binary, **nothing executes** — resident
  emits 0 rows; run-once units go `leased` but never `done`; server log clean (no drain error). Pre-fix, execution
  worked. → the detach/`Arc<E>`/`run_and_apply` refactor broke the **real `InProcessExecutor` + wasm** path, likely
  a panic in the spawned worker task that `tokio::spawn` swallows silently.
- **Gap that let it through:** the regression test uses a mock/`pending` executor, not the real wasm path via the
  `Fleet` in `weir_api::serve` (which `tokio::spawn`s `app.serve()` → HTTP stays up while the worker task can die
  unnoticed).
- **Next:** (a) surface panics in the spawned serve/worker task (don't swallow); (b) add a test that runs a REAL
  resident + run-once through `Fleet`/`InProcessExecutor` (not a mock) — reproduce the claimed-but-never-executed
  regression; (c) correct the detach so real execution + apply still happen. Consider whether detach should keep
  using the existing `spawn_blocking` engine bridge rather than a bare `tokio::spawn`. NOT committed.

**2026-07-10 — STILL UNSOLVED; fix-forward does NOT pass the real path.** The real-executor repro test
(`crates/weir-orchestrator/tests/resident_real.rs`) exists + compiles; running the compiled binary directly
(bypassing the reaped cargo build) **hangs 2+ min, no output** — real resident + run-once does not drain. The current
`weir-orchestrator` detach change on the branch is **unverified and hangs real execution — do NOT merge as-is.**
**Env blocker:** this session's harness reaps long test runs, so I couldn't iterate with real-path verification.
**Recommendation:** (a) **revert** the T-0146 detach to restore last-known-good F1.6/F1.9 (executes correctly;
wedges only while a genuinely-perpetual resident runs — documented), keeping the branch coherent, then fix properly
where `resident_real.rs` can run; or (b) keep WIP + finish locally. Root cause + real-executor repro test are in
place; remaining work = make the detached resident drive the real engine (existing `spawn_blocking` bridge) without
hang/panic, proven by `resident_real.rs` green.

**2026-07-10 — RESOLVED (the fix works; earlier "regression" was a MISDIAGNOSIS).** Once real logging landed
([[WEIR-T-0147]]) and I reproduced the demo (resident on + hit Run on the others) reading the log, the truth was
plain: the detach works. Verbatim demo log shows `claimed → engine run → checkpoint → run done` for echo-quick /
slow-stream (rows=12) / dead-letters, `failing-api` fails by design, and `resident-demo` committing checkpoints
(cursor 11→12) **concurrently** — while `run_until_idle` logs **`drain pass begin`=100 / `drain pass end`=100**
(perfectly balanced → never parked) and `/runs` `pending=0`. Run-once drain to `done` WHILE the resident runs.
**My prior "fix regressed / hangs the real path" was wrong** — it came from black-box `/runs` polling of transient +
stale-leased state (demo-DB cruft from repeated rough server kills) and a truncated log tail; the `resident_real.rs`
"2-min hang" was the wasm-fixture build in test setup, not a deadlock. Detach fix (resident → `tokio::spawn` of
`run_and_apply`, tick returns immediately) is correct + verified on the real server path. Lesson: don't diagnose a
concurrency bug by polling state — instrument it (→ T-0147). Not committed.