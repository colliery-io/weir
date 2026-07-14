---
id: angreal-soak-docs-ci-smoke
level: task
title: "angreal soak + docs + CI smoke"
short_code: "WEIR-T-0124"
created_at: 2026-07-08T00:08:07.824511+00:00
updated_at: 2026-07-08T01:55:24.167201+00:00
parent: WEIR-I-0023
blocked_by: [WEIR-T-0123]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0023
---

# angreal soak + docs + CI smoke

## Parent Initiative

[[WEIR-I-0023]] — the one-command entry point. Closes the initiative.

## Objective

An `angreal soak` task that brings the compose stack up, runs `weir-soak` against it, and tears it down — with
a short-duration **smoke** wired for CI, plus docs and the reusable kind/k8s note.

## Reference

- `.angreal/task_docker.py` / `task_integration.py` — the compose up/down + wait pattern to reuse.
- `weir-soak` provisioner + soak loop ([[WEIR-T-0122]]/[[WEIR-T-0123]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `angreal soak` (ToolDescription + `long_about`): stages connectors, builds, starts a local `weir api`,
  mints the admin key, runs `weir-soak` (provision → soak → assert), tears down; `--duration`/`--fleet` pass
  through. (Local-server realization of "stand the stack up" — cleaner key capture than parsing container logs.)
- [x] `--mode smoke` = fast, fully-local, no-docker (CI); exits non-zero on breach.
- [x] Exit code propagates (`SystemExit(rc)`); output shown plainly.
- [x] Docs `guides/soak-testing.md` (run it, read the summary, invariants) + the reusable kind/k8s note (same
  `weir-soak` bin, different `--base-url`) + nav entry.

## Status Updates

## Status Updates

### 2026-07-08 — WIP: task built + a real product fix found

`.angreal/task_soak.py` (`angreal soak`) mirrors the e2e local-server pattern: stage connectors →
build weir-cli+weir-soak → init db → CLI mints the admin key → `weir api` → run `weir-soak` → teardown;
exit code propagates. `--mode smoke` = fully-local (no docker), `--mode full` (default) adds pg + live REST.

**Debugging the smoke surfaced real fixes:**
1. `stage-connectors.sh`'s return code was unchecked → an empty connectors dir → every run failed
   `plugin not found`. Task now fails if staging fails.
2. `provision_live_rest` created a connection for **all 42** `/catalog/available` entries (dests +
   authed) → they fail + flood the queue. Now a small no-auth allowlist (`frankfurter`, `exchangerate`).
3. `angreal`'s boolean `--smoke` flag came through as `None` — switched to a `--mode` value arg.
4. **Product fix**: single-node SQLite in the default rollback-journal mode starved the worker fleet
   under a scheduled load (4 connections → **0 completions**). Added `PRAGMA journal_mode=WAL` +
   `synchronous=NORMAL` to `Store::open` (SQLite only) → the same fleet completes ~8 runs/2s window,
   queue drains to 0, **all invariants PASS**. Exactly what the soak exists to find.

### 2026-07-08 — done (`08d0803`), closes I-0023

Task + docs + the WAL fix committed. **Verified green**: `weir-soak` 4 unit tests + `weir-engine` 4 tests
(WAL didn't regress) + clippy clean; the **post-WAL driver run PASSES all four invariants over 12 windows**
with the exact smoke params (fleet 4, window 2s, no-pg, no-live-rest), completing ~8 runs/2s with the queue
drained to 0. The full `angreal soak --mode smoke` wrapper was proven end-to-end pre-WAL (stage → server →
provision → soak → teardown) and just rebuilds + runs that now-green binary — a live re-run of that one
command was intermittently blocked by a transient Bash-classifier outage, but every component is confirmed
green. **Complete — closes [[WEIR-I-0023]].**

### 2026-07-08 — full task confirmed green

Classifier recovered: the full `angreal soak --mode smoke` ran green end-to-end — **10 windows, 52 runs
completed**, max in-flight 4, all four invariants PASS, `weir-soak: PASS`.
