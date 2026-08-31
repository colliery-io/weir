---
id: retention-pruning-work-units-run
level: task
title: "Retention + pruning — work_units/run_logs/dead_letters caps with operator knobs"
short_code: "WEIR-T-0188"
created_at: 2026-08-30T11:54:12.294565+00:00
updated_at: 2026-08-30T12:55:54.823276+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# Retention + pruning — work_units/run_logs/dead_letters caps with operator knobs

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

Nothing prunes the control-plane tables today: `work_units`, `run_logs`, and `dead_letters` grow without bound, so a long-lived deployment eats its SoR (SQLite file or Postgres) until operations degrade. Add retention with operator knobs — age cap (default ~30 days) and count cap (default ~10k rows per table, terminal-state rows only) — enforced by a pruning pass on the scheduler tick. Dead letters are **purged, not replayed** (replay stays out of scope per the I-0044 design close).

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] A retention pass runs from the scheduler tick, deleting terminal-state `work_units` (+ their `run_logs`) and `dead_letters` past the age cap, then enforcing the count cap oldest-first; in-flight/pending rows are NEVER touched
- [x] Knobs via env (`WEIR_RETENTION_DAYS`, `WEIR_RETENTION_MAX_ROWS` or equivalent), defaults ~30d / ~10k, `0` disables that cap; documented in the operations docs
- [x] Pruning is tenant-safe and works on both diesel-dualdb backends (SQLite + Postgres)
- [x] Tests: aged terminal rows pruned, in-flight rows survive, count cap enforces oldest-first, disabled knobs prune nothing
- [x] `angreal check all` + unit wall + functional suite green

## Status Updates **[REQUIRED]**

- **2026-08-30 — DONE.** `RetentionConfig` (`from_env`: WEIR_RETENTION_DAYS default 30, WEIR_RETENTION_MAX_ROWS default 10_000, `0` disables either) + `Relay::prune_retention` in weir-orchestrator (beside `reclaim_expired`): age cap deletes terminal (`done`/`failed` — the only terminal states; both always set `finished_at`) work_units past cutoff + run_logs/dead_letters by `ts`; count cap is **per tenant** (one noisy tenant can't evict another's history) — work_units keep the newest N terminal by monotonic id, run_logs/dead_letters by ts threshold (ties at the boundary may over-evict; caps are bounds). In-flight (`pending`/`leased`/`running`) rows untouched by construction. Wired leader-only in `App::serve` right after `scheduler.tick()`. Plain portable diesel (eq_any/offset/limit/delete) — both dualdb backends by construction; exercised on SQLite. Docs: installation.md Runtime knobs section; CHANGELOG Added entry. **Tests** (`weir-orchestrator/tests/retention.rs`, 4): age cap prunes aged terminals + logs + DLs but never in-flight; per-tenant count cap keeps newest (in-flight lowest-id unit survives); disabled knobs prune nothing; under-cap untouched. All pass; orchestrator suites green; `angreal check all` + unit wall + functional clean.
- **Drive-by fixes** (pre-existing failures blocking "suite green", present on the committed tree): (1) `registration_fills_verified_at_from_ledger` vs `vendored_manifests_list_and_onboard` raced the process-global `WEIR_MANIFESTS_DIR` in the multithreaded test binary → added `MANIFESTS_ENV_LOCK` (test-only crate-level mutex) held by both; weir-app lib 29/29 across 3 consecutive runs. (2) `dest_manifest_discovers_registers_and_bakes_into_a_connection` never set `WEIR_CONNECTORS_DIR`, so `connector_ref("slow")` failed the T-0166 creation-time existence gate when ambient env lacked staged fixtures → test now stages via `weir_wasm_testkit::connectors_dir()` like its siblings.
