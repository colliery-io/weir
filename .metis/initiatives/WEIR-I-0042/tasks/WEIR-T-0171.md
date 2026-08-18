---
id: schedule-fidelity-re-register-on
level: task
title: "Schedule fidelity — re-register on config change; schedule all tenants"
short_code: "WEIR-T-0171"
created_at: 2026-08-16T15:24:10.176153+00:00
updated_at: 2026-08-16T15:24:10.176153+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Schedule fidelity — re-register on config change; schedule all tenants

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Two schedule-reconciler traps. (1) Editing a connection's config without changing cadence keeps firing the OLD spec — schedules.spec snapshots the WorkSpec at registration and sync_schedules re-registers only when (every_ms, cron) changed — so a user who fixes a bad credential watches it keep failing: the definitional mysterious failure. (2) sync_schedules hardcodes DEFAULT_TENANT, so scheduled connections owned by any other tenant never fire under serve/api even though the API sells the tenancy surface.

## Evidence (2026-08-16 alpha review)

- `crates/weir-app/src/lib.rs:1141` — re-register condition is cadence-only.
- `crates/weir-app/src/lib.rs:1114-1117` — `let tenant = DEFAULT_TENANT` in sync_schedules.
- `Scheduler::schedules()/remove()` are not tenant-scoped, and `Relay::cancel`/`has_active` filter by connection name only (`crates/weir-orchestrator/src/lib.rs:967-1011`) — cross-tenant name collisions are live hazards once tenant iteration lands.
- `crates/weir-app/tests/serve.rs` — existing injected-clock tests, all default-tenant only.

## Acceptance Criteria **[REQUIRED]**

- [ ] A config edit that keeps the cadence refreshes the registered schedule spec; the next fire uses the new config — proven by an injected-clock serve.rs test
- [ ] sync_schedules reconciles every tenant with tenant-scoped schedule identity — OR, if deliberately deferred, serve logs the single-tenant limitation at startup and the docs state it; either way the decision is recorded in this task
- [ ] If multi-tenant scheduling lands: a serve.rs test proves a non-default tenant's cadence fires, and cross-tenant name-collision behavior for cancel/has_active is at minimum characterized in a test (full fix may hand off to the multi-tenant hardening initiative)

## Implementation Notes

Cheapest config-refresh fix: compare a hash of the built WorkSpec (not just cadence) in sync_schedules and re-register on any change. For tenant scoping, WEIR-A-0036 (per-tenant runner fleet) already establishes composite (tenant_id, name) identity — follow that convention for schedule keys. Full cross-tenant cancel/has_active hardening belongs to the later multi-tenant initiative; here only avoid making collisions worse and characterize current behavior.

## Status Updates **[REQUIRED]**

*To be added during implementation*
