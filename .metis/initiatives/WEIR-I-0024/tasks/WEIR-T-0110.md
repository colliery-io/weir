---
id: aggregate-health-endpoints-health
level: task
title: "Aggregate health endpoints + health computation"
short_code: "WEIR-T-0110"
created_at: 2026-07-07T04:06:01.267987+00:00
updated_at: 2026-07-07T04:39:01.376188+00:00
parent: WEIR-I-0024
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0024
---

# Aggregate health endpoints + health computation

## Parent Initiative

[[WEIR-I-0024]] — the data layer both views read.

## Objective

Compute per-connection **health** (green/amber/red) from the store and expose it via tenant-scoped endpoints,
plus a **platform-admin** cross-tenant rollup + fleet status. Authoritative + row-exact ([[WEIR-A-0036]] tenancy).

## Reference

- `crates/weir-api/src/lib.rs` — the `/tenants/{id}/*` handlers ([[WEIR-T-0094]]) + `tenant_of`; add the health
  endpoints alongside. `authz.rs` for platform-admin gating.
- Tables: `runs` (status/started/finished), `work_units` (state/next_attempt_at → queue depth), `dead_letters`,
  `stream_state` (cursor), `schedules` (every_ms/cron → expected cadence).
- `Relay::pending_depth` ([[WEIR-T-0104]]) + `active_tenants` for queue/fleet signals.

## Health model (v1, fixed thresholds)

Per connection, roll up to a status:
- **freshness** — last successful run within `schedule + grace` (unscheduled = last-run-based);
- **error-rate** — recent failed/total over a threshold;
- **dead-letters** — present / climbing.
`red` if any hard signal trips, `amber` on soft/stale, else `green`. Tenant status = worst of its connections.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] Pure `health::compute` → `ConnectionHealth` (status/last_success/lag/error_rate/dead_letters/throughput);
  5 unit tests (green/amber/red/unknown/worst).
- [x] `GET /overview` (implicit own-tenant) + `GET /tenants/{id}/overview` (explicit, admin) — per-connection health.
- [x] `GET /platform/health` — platform-admin cross-tenant rollup (per-tenant status + needs-attention + queue/
  fleet). Non-admin → 403 (handler test).
- [x] Builds; clippy clean; handler test covers tenant-scoping + the admin gate.

## Status Updates

### 2026-07-07 — implemented, build pending (Bash classifier flaky)

- **`weir-app/src/health.rs`** — pure `compute(connection, runs, dead_letters, schedule_ms, now, thresholds)
  → ConnectionHealth { status, last_success_ms, lag_ms, recent_total/failed, error_rate, dead_letters,
  rows_recent, throughput }`. State-string-agnostic (terminal = not pending/leased/running; success = `done`;
  terminal-not-done = failure). green/amber/red + **Unknown** (no terminal runs). Thresholds: 60s grace,
  amber@0.2 / red@0.5 error-rate, 2× window = red staleness. `worst()` for rollups. Platform types
  (`TenantHealth`/`AttentionItem`/`PlatformHealth`). **5 unit tests pass** (green/amber/red/unknown/worst).
- **`App::tenant_health(tenant, now)`** — one `work_units` query (recent, grouped per connection, cap 20) +
  dead-letter tally + `list_connections` schedule → `compute` per connection.
- **`App::platform_health(now)`** — over `list_tenants`: per-tenant `worst()` status + counts + queue depth
  (`pending_depth`) + cross-tenant needs-attention (red-first) + active-tenant fleet count.
- **Endpoints** (`weir-api`): `GET /overview` (implicit-tenant, `Any`, handler-scoped), `GET /tenants/{id}/overview`
  (platform-admin), `GET /platform/health` (platform-admin rollup). All classified in `authz.rs` (own = Any,
  cross-tenant + platform = `Scope::Platform`).

### 2026-07-07 — done (`1b05bc0`)

Bash recovered → weir-app+weir-api build clean, clippy clean. Handler test
**`health_overview_scoped_and_platform_gated`**: `/overview` returns the tenant's connection (status
`unknown`, no runs yet); `/platform/health` is **403 for a non-admin** key, **200 for admin** with the default
tenant in the rollup. **Complete** — the data layer for both ops views ([[WEIR-T-0111]]/[[WEIR-T-0112]]).
