---
id: turn-retries-on-in-production
level: task
title: "Turn retries on in production — WorkerConfig knobs, default max_attempts=3"
short_code: "WEIR-T-0169"
created_at: 2026-08-16T15:24:07.314615+00:00
updated_at: 2026-08-16T15:24:07.314615+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


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

- [ ] Production default for serve/api/runner: max_attempts=3 with exponential backoff and a sane base delay
- [ ] `WEIR_MAX_ATTEMPTS` / `WEIR_RETRY_BASE_MS` env knobs (CLI flags optional) honored by all three entrypoints
- [ ] Fatal errors still fail immediately; exhausted transients still land failed with the error visible in the run feed — existing semantics preserved
- [ ] Test covering env → WorkerConfig wiring; docs reference page notes defaults + knobs

## Implementation Notes

Decide whether changing `WorkerConfig::default()` itself (affects tests that rely on max_attempts=1) or building the production config explicitly in weir-app's serve/runner paths — the latter is safer and keeps test semantics untouched. Mind the interaction with resident supervision: resident restart attempts share the attempt counter (review noted no backoff decay/cap there — out of scope here, but don't make it worse).

## Status Updates **[REQUIRED]**

*To be added during implementation*
