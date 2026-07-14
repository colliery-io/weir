---
id: per-tenant-runner-claim-by-tenant
level: task
title: "Per-tenant runner + claim-by-tenant + fleet coordinator"
short_code: "WEIR-T-0091"
created_at: 2026-07-05T23:57:23.104496+00:00
updated_at: 2026-07-06T00:41:44.123202+00:00
parent: WEIR-I-0018
blocked_by: [WEIR-T-0089]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0018
---

# Per-tenant runner + claim-by-tenant + fleet coordinator

## Parent Initiative

[[WEIR-I-0018]] — the execution-isolation core. Governed by [[WEIR-A-0036]] (decision 1: physical per-tenant runners).

## Objective

Make execution physically tenant-isolated: a runner has a **tenant identity** and `claim()` only ever leases
**its** tenant's work; a **fleet coordinator** ensures a runner per active tenant. This is the in-process /
single-node runtime that [[WEIR-I-0021]] later provisions + autoscales on k8s.

## Reference

- `crates/weir-orchestrator/src/lib.rs` — `Executor::claim()` (state-guarded FIFO `UPDATE`, ~L414),
  `heartbeat`, `load`; the lease model tolerates many independent claimants ([[WEIR-A-0010]]/[[WEIR-A-0011]]).
- cloacina `~/Desktop/cloacina/crates/cloacina-server/src/`: `tenant_runner_cache.rs`, `fleet_coordinator.rs`,
  `fleet_executor.rs` — the per-tenant runner fleet to mirror.
- The scheduler/worker is spawned in-process by `App::serve` today ([[WEIR-A-0028]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `claim(owner, lease)` → `claim(owner, tenant_id, lease)`: the candidate query filters
  `WHERE tenant_id = ? AND state='pending' AND next_attempt_at <= now`, ordered by id (per-tenant FIFO); the
  state-guarded UPDATE is unchanged. Uses the `(tenant_id, state, next_attempt_at)` index ([[WEIR-T-0089]]).
- [ ] A runner carries a `tenant_id`; it never observes another tenant's `work_units`.
- [ ] A **fleet coordinator** discovers active tenants (those with pending/scheduled work) and ensures a runner
  each; idle-tenant runners are reaped. Fairness: no tenant starves others (round-robin / per-tenant runner).
- [ ] Scheduling/enqueue stamps `tenant_id` onto `work_units` from the connection's tenant.
- [ ] Tests: a two-tenant store where tenant-A's runner only ever claims tenant-A units; workspace + clippy clean.

## Implementation Notes

- Decide eager (a runner per active tenant, spun on tenant activity) vs lazy (spawn on first pending) — see the
  initiative's open sub-question; lazy-with-reaping is likely simpler for the in-process case.
- Keep the fleet-coordinator abstraction thin + provider-agnostic so [[WEIR-I-0021]] swaps the in-process
  runner spawn for a k8s pod without touching claim/lease logic.

## Status Updates

### 2026-07-06 — done (`0f2d051`)

- **claim-by-tenant** (`Relay::claim(owner, tenant, lease)`) — candidate query filters
  `tenant_id = ? AND state='pending' AND next_attempt_at <= now`, per-tenant FIFO on the
  `(tenant_id,state,next_attempt_at)` index ([[WEIR-T-0089]]); the atomic state-guarded UPDATE is unchanged.
- **Runner tenant identity** — `WorkerConfig` gains `tenant`; `Worker::tick` passes it; a worker only ever
  observes its own tenant's `work_units`.
- **Fleet** (`weir-orchestrator`) — `Relay::active_tenants()` (distinct tenants with due work) + `Fleet::
  run_until_idle`: each cycle runs a **per-tenant `Worker` to idle for every active tenant**. Idle tenants
  aren't returned → get no runner (**natural reaping**); each tenant has its own worker → **no starvation**.
  Thin + provider-agnostic (a `make_executor` factory) so [[WEIR-I-0021]] swaps the in-process worker for a
  k8s pod without touching claim/lease. `App::drain` + `App::serve` now drive the `Fleet`.
- Enqueue already stamps `tenant` (via `WorkSpec.tenant` → `work_units.tenant_id`, [[WEIR-T-0090]]).

**Chose lazy-with-reaping** (per-poll rediscovery) over long-lived per-tenant tasks — simplest for in-process,
and the reaping falls out for free. Test **`claim_is_tenant_isolated`**: acme's runner claims only acme's unit
(then nothing), globex's runner claims only globex's; `active_tenants` lists both. Workspace + all suites +
clippy green. **Complete.**
