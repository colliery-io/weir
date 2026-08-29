---
id: turn-retries-on-in-production
level: task
title: "Turn retries on in production — WorkerConfig knobs, default max_attempts=3"
short_code: "WEIR-T-0169"
created_at: 2026-08-16T15:24:07.314615+00:00
updated_at: 2026-08-25T03:07:11.968688+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Turn retries on in production — WorkerConfig knobs, default max_attempts=3

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Production runs with retries off: serve/api/runner build `WorkerConfig::default()` (max_attempts=1, base_delay=0) and expose no knob, so every transient failure — rate limit, network blip — is a terminal red run until the next scheduled fire. The retry machinery (transient → backoff requeue, fatal → immediate fail) is fully built and tested; turn it on with sane defaults and knobs.

## Evidence (2026-08-16 alpha review)

- `crates/weir-orchestrator/src/lib.rs:359-371` — `WorkerConfig::default` max_attempts=1 (verified at :364).
- `crates/weir-app/src/lib.rs:1190-1193, 1248-1251` — serve/runner use the default; grep found no setter outside tests.
- Machinery proven: `crates/weir-orchestrator/tests/orchestrator.rs` — persistent_transient_exhausts_to_failed, fatal_fails_without_retry.

## Acceptance Criteria **[REQUIRED]**

- [x] Production default for serve/api/runner: max_attempts=3, exponential backoff, base 1000ms — via `production_worker_config()` used by `App::serve` (which backs both `weir serve` and `weir api`) and `App::run_workers` (`weir runner`)
- [x] `WEIR_MAX_ATTEMPTS` / `WEIR_RETRY_BASE_MS` env knobs honored by all three entrypoints (CLI flags deliberately skipped — env matches the container/k8s posture)
- [x] Fatal errors still fail immediately; exhausted transients land failed with the error in the run feed — `WorkerConfig::default()` untouched (tests + the one-shot drain keep max_attempts=1, so `run_until_idle` never returns with a delayed retry pending)
- [x] `worker_config_tests` cover knob parsing (defaults / overrides / garbage / zero-clamp) and the production config; `docs/reference/installation.md` gains a "Runtime knobs" section

## Implementation Notes

Decide whether changing `WorkerConfig::default()` itself (affects tests that rely on max_attempts=1) or building the production config explicitly in weir-app's serve/runner paths — the latter is safer and keeps test semantics untouched. Mind the interaction with resident supervision: resident restart attempts share the attempt counter (review noted no backoff decay/cap there — out of scope here, but don't make it worse).

## Status Updates **[REQUIRED]**

**2026-08-25 — implemented + tested (ralph run).**

- `crates/weir-app/src/lib.rs`: new `retry_knobs()` (parses/defaults the env values, garbage-safe, attempts clamped ≥1) + `production_worker_config(concurrency)` used at the two daemon fleet sites (`App::serve` L~1229, `App::run_workers` L~1287). The one-shot drain path (`run_until_idle`, used by `weir run`) deliberately stays on `WorkerConfig::default()` — a delayed transient requeue there would outlive the drain and silently strand the retry, so one-shot semantics are unchanged.
- Resident interaction unchanged: supervision's requeue path is orthogonal to `max_attempts` for perpetual units; no backoff-decay behavior touched.
- Tests: `worker_config_tests::{retry_knobs_defaults_overrides_and_garbage, production_config_carries_retries_and_concurrency}`; full weir-app lib 28/28, serve 2/2, cli 8/8, `angreal check all` clean.
- Docs: `docs/reference/installation.md` "Runtime knobs" section (defaults + knobs + fatal-vs-transient statement).
