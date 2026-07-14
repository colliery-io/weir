---
id: leader-elected-autoscaler-on-queue
level: task
title: "Leader-elected autoscaler on queue depth"
short_code: "WEIR-T-0104"
created_at: 2026-07-06T10:43:11.742783+00:00
updated_at: 2026-07-06T11:02:17.873208+00:00
parent: WEIR-I-0021
blocked_by: [WEIR-T-0102, WEIR-T-0103]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0021
---

# Leader-elected autoscaler on queue depth

## Parent Initiative

[[WEIR-I-0021]]. Governed by [[WEIR-A-0023]] (weir-owned, store-leader, queue-depth, scale-to-zero).

## Objective

A **leader-elected** controller that scales each tenant's runner Deployment on **queue depth** — up under load,
**down to zero** when idle — so only tenants with work run pods.

## Reference

- cloacina `~/Desktop/cloacina/crates/cloacina-server/src/autoscaler/{leader,mod}.rs` — leader-elected scaling
  to mirror; leader via a store row (portable, no k8s dependency).
- Signal: `Relay::active_tenants` ([[WEIR-T-0091]]) + a new `Relay::pending_depth(tenant)` (count of due pending
  work) — this also lands the deferred **`weir_queue_depth` gauge** ([[WEIR-T-0099]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `Relay::pending_depth(tenant)` (+ the `weir_queue_depth{tenant}` gauge emitted periodically).
- [ ] A leader-election over a **store row** (a `leases`/`autoscaler_leader` row with an owner + expiry,
  heartbeated) — only the leader acts; portable (works without k8s).
- [ ] The leader loops: for each active tenant, compute a target replica count from queue depth
  (`0` when idle → scale-to-zero; a bounded ramp under load) and call the `Actuator::scale(tenant, n)`
  ([[WEIR-T-0103]]); idle tenants → `remove`/scale-0.
- [ ] Config: min/max replicas, the depth→replicas policy, poll interval; a switch to defer to HPA/KEDA.
- [ ] Unit tests: the depth→replicas policy + single-leader guarantee; workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`9655bdf`)

- `Relay::pending_depth(tenant)` (count of due pending work) — the autoscaler signal; the `tick` emits the
  **`weir_queue_depth{tenant}` gauge** (the deferred [[WEIR-T-0099]] metric).
- `Relay::try_acquire_lease(name, owner, ttl)` — **atomic store-based leader election** (new `leader_leases`
  table in the baseline schema; regen'd, migrates on both backends): take if expired-or-ours (state-guarded
  UPDATE) else insert-or-lose. Portable — no k8s dependency.
- `weir-orchestrator/autoscaler.rs`: `ScalePolicy` (depth→replicas: `0` idle → scale-to-zero, else
  `ceil(depth/per_replica)` clamped `[min,max]`) + `Autoscaler::tick` — **if leader**, scale each active tenant
  via the `Actuator` ([[WEIR-T-0103]]) and **scale-to-zero** tenants that went idle (tracked set).
- Deferring to HPA/KEDA = simply not running the autoscaler (a `values.yaml` switch, [[WEIR-T-0105]]).

Tests: **policy** (scale-to-zero + bounded ramp) + **single-leader guarantee** (A holds, B blocked, expiry →
B takes over). Workspace + dual-backend migrate + clippy green. Live scale is the kind smoke ([[WEIR-T-0105]]).
**Complete.**
