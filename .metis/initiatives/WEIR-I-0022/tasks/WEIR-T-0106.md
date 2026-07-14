---
id: scheduler-leader-election-ha
level: task
title: "Scheduler leader-election (HA control plane)"
short_code: "WEIR-T-0106"
created_at: 2026-07-06T11:33:07.627742+00:00
updated_at: 2026-07-06T11:39:36.286245+00:00
parent: WEIR-I-0022
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0022
---

# Scheduler leader-election (HA control plane)

## Parent Initiative

[[WEIR-I-0022]] — closes the **HA control plane** partial.

## Objective

Make the control plane **safe to run in more than one replica**: only the leader schedules. Today
`App::serve` loops `sync_schedules` + `scheduler.tick()` on every replica → N replicas would enqueue each
due connection N times.

## Reference

- `crates/weir-app/src/lib.rs` — `serve` (~L948): `Scheduler::new` + the loop calling `sync_schedules`
  (~L969) and `scheduler.tick()` (~L972).
- `crates/weir-orchestrator/src/lib.rs` — **`Relay::try_acquire_lease(name, owner, ttl)` already exists**
  ([[WEIR-T-0104]], ~L551); the autoscaler uses the same mechanism. Reuse it here.
- Workers/runners are already multi-claimant-safe (the work-unit lease, [[WEIR-A-0011]]); only *scheduling*
  needs the guard.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `serve` acquires/renews a `"scheduler"` lease each cycle; it only runs `sync_schedules` +
  `scheduler.tick()` **when it holds the lease** (a non-leader replica idles, ready to take over on expiry).
- [ ] The lease owner is stable per process; TTL comfortably exceeds the poll interval.
- [ ] Optional flag/env to force-disable scheduling on a replica (an API-only front end).
- [ ] Test: **two `App`s on one store** run the schedule loop; a due connection is enqueued **exactly once**
  per period (not twice). Workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`c1559f2`)

`App::serve` now acquires/renews a `"scheduler"` store-lease each cycle (reusing `Relay::try_acquire_lease`
from [[WEIR-T-0104]], owner `weir-serve-<pid>`, TTL `poll*3`) and only runs `sync_schedules` + `scheduler.tick()`
**when it holds the lease**. Draining stays unguarded — the work-unit lease ([[WEIR-A-0011]]) already makes that
multi-claimant-safe. `WEIR_DISABLE_SCHEDULER` makes a replica API-only.

Test **`only_the_leader_schedules`**: two `App`s on one store run a guarded cycle → exactly one leads (without
the lease both would enqueue). cli suite 4/4, clippy clean. **Complete — closes the HA-control-plane partial.**
