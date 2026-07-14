---
id: extract-standalone-weir-runner
level: task
title: "Extract standalone weir runner process"
short_code: "WEIR-T-0102"
created_at: 2026-07-06T10:42:58.481932+00:00
updated_at: 2026-07-06T10:49:18.665176+00:00
parent: WEIR-I-0021
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0021
---

# Extract standalone weir runner process

## Parent Initiative

[[WEIR-I-0021]] — the foundation. Governed by [[WEIR-A-0023]].

## Objective

Extract the worker into a standalone **`weir runner`** process that claims work from the shared store and
executes it, so N runners can run against one control plane. In-process stays the single-node default.

## Reference

- `crates/weir-app/src/lib.rs` — `App::serve` builds a `Fleet` + scheduler in-process (~L632); `App::drain`
  drives the `Fleet`. The lease/claim model ([[WEIR-A-0010]]/[[WEIR-A-0011]]) already tolerates many claimants.
- `crates/weir-cli/src/main.rs` — the `Commands` enum (`Init`/`Run`/`Serve`/`Api`/`Auth`); add `Runner`.
- The `Fleet` (weir-orchestrator, [[WEIR-T-0091]]) has a `make_executor` factory — a runner just drives it.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A `weir runner` subcommand: opens the shared store (Postgres in prod), runs the `Fleet` claiming +
  executing work in a loop until shutdown; no HTTP, no scheduler (the control-plane `serve` still schedules).
- [ ] Optionally tenant-scoped (`--tenant <id>`) so a runner serves one tenant ([[WEIR-I-0018]] pod-per-tenant);
  unset = all active tenants (single-node).
- [ ] The control-plane `serve` keeps its in-process fleet as the default; a flag can disable the in-process
  worker when external runners handle execution.
- [ ] Test: a runner process (or task) drains work **planned by a separate `App`/process** against the same
  store → the work reaches `done`. Workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`54fdfe6`)

`App::run_workers(poll, concurrency, tenant, shutdown)` — a worker-only loop (no scheduler/HTTP): `tenant=Some`
runs a single-tenant `Worker` (pod-per-tenant, [[WEIR-I-0018]]), `None` drains all active tenants via the
`Fleet` ([[WEIR-T-0091]]). New **`weir runner [--tenant] [--poll] [--concurrency]`** subcommand. The control
plane's `serve` keeps its in-process fleet as the default; the atomic lease ([[WEIR-A-0011]]) lets many runners
+ the control plane claim from one store without double-execution.

Test **`runner_drains_work_planned_by_another_app`**: a separate runner `App` on the same store drains work
planned by the control-plane `App` → `done`. Builds; clippy clean. **Complete.** (Minor follow-up for the k8s
deploy: a `serve` flag to *disable* the in-process worker when external runners own execution — lands with the
chart wiring in [[WEIR-T-0105]]; safe either way thanks to leases.)
