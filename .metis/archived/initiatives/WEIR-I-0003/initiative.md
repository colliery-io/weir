---
id: hardening-the-migratable-core
level: initiative
title: "Hardening the Migratable Core"
short_code: "WEIR-I-0003"
created_at: 2026-06-19T13:44:36.477202+00:00
updated_at: 2026-06-19T18:02:46.244413+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: hardening-the-migratable-core
---

# Hardening the Migratable Core Initiative

## Context **[REQUIRED]**

Phase 1 ([[WEIR-I-0002]]) shipped the Migratable Core — a runnable single-node `weir` (CLI + `serve` + control-plane API + embedded Dioxus UI) over the contract / dual-primitive execution / orchestration spine. It is a *good start*, with known corners cut for velocity. This initiative pays down the highest-value robustness and structural debt so the core is dependable before Phase 2 (connector breadth / out-of-process worker fleet) builds on it.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- Close the lease/heartbeat correctness gap so long-running syncs can't be double-executed (de-risks the agent-fleet model).
- Collapse the two parallel run-tracking paths into a single source of truth.
- Clean up crate structure (extract `weir-app`) so the HTTP API is its own crate, not folded into `weir-cli`.
- Extend scheduling beyond fixed intervals (cron).
- Make CI correct (green) so it's ready to re-enable on going public.

**Non-Goals:**
- `wasi:http` for WASM connectors — blocked on an upstream fidius capability FR; the weir-side consume work is ticketed ([[WEIR-T-0023]]) but stays in backlog until unblocked.
- Out-of-process worker fleet, the Dispatcher multi-executor router, and backfill upper-bound (`to`) — these belong to Phase 2.
- New connectors or UI features.

## Detailed Design **[REQUIRED]**

Each task is a self-contained, test-gated change to the existing crates (`weir-orchestrator`, `weir-engine`, `weir-cli`, `weir-ui`). No new architectural decisions are required (no ADRs); this is hardening within the spine decided in [[WEIR-I-0001]] / [[WEIR-A-0028]].

- **Heartbeat** ([[WEIR-T-0018]]): the `Worker` extends its lease (`lease_expires_at`) on a heartbeat interval while a unit executes, so a sync longer than the lease isn't reclaimed by `reclaim_expired`. Validated with the `slow` connector (`sleep_ms` > lease).
- **Run reconciliation** ([[WEIR-T-0019]]): make the relay `work_units` the single run record; fold or retire `weir-engine::RunManager`'s parallel `runs` table so the API/UI and engine agree.
- **Cron** ([[WEIR-T-0020]]): extend the `Scheduler` with cron expressions (same relay `plan` path, cron-derived `next_due_at`).
- **`weir-app` extraction** ([[WEIR-T-0021]]): move `App` + config model into a `weir-app` crate; `weir-api` becomes its own crate over it; `weir-cli` (bin) depends on both — resolving the dependency cycle worked around in Phase 1.
- **CI** ([[WEIR-T-0022]]): fix `ci.yml`/`docs.yml` so they pass; keep Actions disabled while private but correct for re-enable.

## Alternatives Considered **[REQUIRED]**

- **Do nothing / fold into Phase 2.** Rejected: the heartbeat and run-tracking gaps are correctness issues that Phase 2's fleet would amplify; cheaper to fix on the small current surface.
- **`weir-api` crate first (before other tasks).** Rejected as the lead: it's the riskiest churn; sequence it after the correctness fixes so they land on a stable structure.

## Implementation Plan **[REQUIRED]**

Sequenced by value/risk; each task lands as its own commit set, tests green:
1. [[WEIR-T-0018]] lease heartbeat (correctness — first)
2. [[WEIR-T-0019]] reconcile run tracking
3. [[WEIR-T-0020]] cron schedules
4. [[WEIR-T-0021]] `weir-app` / `weir-api` extraction
5. [[WEIR-T-0022]] fix CI workflows
- [[WEIR-T-0023]] WASM connector outbound HTTP — **backlog, blocked** on the upstream fidius `wasi:http` FR.

## Exit Criteria

- [ ] Heartbeat prevents reclaim of an in-flight unit; proven with a sync longer than the lease.
- [ ] One run record (relay) backs CLI, API, and UI; the parallel `RunManager` table is gone or explicitly bridged.
- [ ] `weir-api` is its own crate over `weir-app`; `weir-cli` is a thin bin; no dependency cycle.
- [ ] Scheduler supports cron expressions (interval still works).
- [ ] CI workflows pass green (kept disabled while private).
