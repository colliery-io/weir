---
id: observability-instrument-worker
level: task
title: "Observability: instrument worker/engine lifecycle events (unblock T-0146 diagnosis)"
short_code: "WEIR-T-0147"
created_at: 2026-07-10T21:48:23.535688+00:00
updated_at: 2026-07-10T21:49:01.246980+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# Observability: instrument the worker/engine lifecycle

## Parent Initiative

[[WEIR-I-0035]] (F1). We could not diagnose [[WEIR-T-0146]] (resident wedge / stuck `pending`) from the demo because
the execution path is **effectively silent**. This unblocks it (and ops in general).

## Finding

`init_tracing()` runs for every command incl. `api` (weir-cli:245), RUST_LOG-driven (default `info`), JSON→stderr.
But the fmt layer prints **events only** (no span enter/exit), and the code emits almost none: **`weir-engine` 0
tracing events, `weir-orchestrator` 2.** So the `run` span logs nothing and the whole claim→execute→done +
resident + drain + scheduler flow is invisible. Errors are `eprintln!` (not tracing), so they don't carry context.

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Orchestrator** events (target `weir_orchestrator`): `Relay::claim` (unit id, connection, mode, attempt);
  `run_until_idle` pass start/end (+ claimed/processed counts); resident **detach/spawn**, requeue+backoff (delay,
  attempt), gate/release; `reclaim_expired` (count); scheduler `tick` (schedules fired); leader lease acquire/lose.
- [ ] **Engine** events (target `weir_engine`): run start (connection, mode), each **Checkpoint commit** (rows,
  dead-letters), resident stream **start/stop/end**, terminal outcome/error.
- [ ] **Convert `eprintln!` → `tracing::warn!/error!`** with fields (so "fleet drain failed" / "resident run task
  error" carry unit/connection + the error).
- [ ] Levels sane: lifecycle at `debug`, milestones + errors at `info`/`warn`/`error`; a `RUST_LOG=weir=debug` run
  shows the full claim→execute→done + resident story. `WEIR_LOG_FORMAT=pretty` gives readable local output.
- [ ] Optional: add `.with_span_events(FmtSpan::NEW|CLOSE)` (or gate via env) so the `run` span open/close shows.
- [ ] No perf regression on the hot path (events are cheap; avoid per-record logging — per-checkpoint/per-poll ok).

## Implementation Notes

- Files: `crates/weir-orchestrator/src/lib.rs` (Relay/Worker/Fleet/Scheduler), `crates/weir-engine/src/lib.rs`
  (drive/run_resident/sync_with), and `crates/weir-cli/src/main.rs` (optional span-events).
- Then **use it**: run `RUST_LOG=weir_orchestrator=debug,weir_engine=debug WEIR_LOG_FORMAT=pretty weir … api` for the
  demo, turn on the resident, hit Run on the others, and read the log to pinpoint T-0146.

### Verification
- `cargo build --workspace`, fmt/clippy; run the demo with logging and paste a log excerpt showing claim→execute→
  done for a run-once AND the resident lifecycle. No test hang required (this is instrumentation).

## Status Updates

**2026-07-10 — DONE (instrumentation added + demonstrated).** Files: `weir-engine/Cargo.toml` + `weir-app/Cargo.toml`
(added `tracing` dep), `weir-engine/src/lib.rs`, `weir-orchestrator/src/lib.rs`, `weir-app/src/lib.rs`.
Events added — orchestrator (`weir_orchestrator`): claimed unit (unit/connection/resident/attempt), run_until_idle
drain pass begin/end (+processed), resident detach (info), requeue-with-backoff (info/warn +delay/attempt), run
done/failed, reclaim_expired (count), scheduler tick (schedules_fired); engine (`weir_engine`): run start
(connection/resident), checkpoint commit (cursor/batches), resident-stream-end (warn), connector fatal (error);
weir-app serve loop: `eprintln!`→`tracing::warn/error` + scheduler leadership-change debug.
Verify: `cargo build` green; fmt clean; clippy clean; lib tests weir-engine 14 / weir-orchestrator 8.
Demonstrated (verbatim): server path logs `scheduler tick … schedules_fired=1`, `run_until_idle: drain pass begin`,
`claimed unit … connection=obs-echo resident=false`; one-shot `weir run` logs `engine run: draining read stream` →
`checkpoint commit … batches=1` → `run done … rows=1 dead=0`.

**FINDING (surfaced by the new logs, hands the parent the T-0146 lead):** in the `weir api` SERVER path, a claimed
unit logs `claimed unit` but then **no `engine run` / `checkpoint` / `run done`**, and `run_until_idle` logs
`drain pass begin` with **no `drain pass end`** → the serve-loop drain is **parked and execution never reaches the
engine**. The SAME db drains fully via one-shot `weir run` (engine events + `run done` all fire). So the
fix-forward regression is in the **server serve-loop / detach path**, not the engine — reproducible now with
`RUST_LOG=weir_orchestrator=debug,weir_engine=debug`.