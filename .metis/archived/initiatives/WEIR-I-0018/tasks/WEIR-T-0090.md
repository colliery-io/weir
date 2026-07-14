---
id: tenant-scoped-store-tenants-routes
level: task
title: "Tenant-scoped store + /tenants/* routes + authz"
short_code: "WEIR-T-0090"
created_at: 2026-07-05T23:57:19.934267+00:00
updated_at: 2026-07-06T00:33:07.989007+00:00
parent: WEIR-I-0018
blocked_by: [WEIR-T-0089]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0018
---

# Tenant-scoped store + /tenants/* routes + authz

## Parent Initiative

[[WEIR-I-0018]]. Governed by [[WEIR-A-0036]] (decision 3: implicit tenant + explicit `/tenants/{id}/*`).

## Objective

Make the API tenant-aware: every store method + handler scopes to the caller's `tenant_id` (from
`AuthenticatedKey`), a platform-admin manages tenants + their keys via explicit `/tenants/{id}/*` routes, and the
authz route table moves connection/catalog routes to tenant scope. Single-tenant callers (implicit `default`)
are unaffected.

## Reference

- `crates/weir-app` — the store methods (connections/catalog/runs CRUD); `AuthenticatedKey{tenant_id,is_admin}`
  already exists ([[WEIR-T-0084]]).
- `crates/weir-api` — handlers + `authz.rs` (`Scope`/`evaluate`/`build_authz_table`, [[WEIR-T-0085]]); the
  `Extension<AuthenticatedKey>` is already on every authed request.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] Every scoped store method takes/filters by `tenant_id`; a caller can only read/create/mutate/delete its
  own tenant's connections, catalog entries, runs, dead-letters. Cross-tenant access returns 404 (not 403 —
  don't leak existence).
- [ ] Handlers derive `tenant_id` from `Extension<AuthenticatedKey>` (implicit); no ambient header.
- [ ] `/tenants` (list/create/delete) + `/tenants/{id}/keys` (mint/list/revoke a tenant's keys) —
  **platform-admin only** via `Scope::Platform`; `authz.rs` classifies them.
- [ ] `authz.rs`: connection/catalog/run routes move to tenant scope; `evaluate`'s tenant branch is exercised;
  a read-key of tenant A gets 404 on tenant B's resources.
- [ ] Mutations still write audit rows ([[WEIR-T-0085]]) with the tenant in the actor/resource.
- [ ] Existing single-tenant tests pass under the implicit `default` tenant; new tenant-scoping unit/api tests; clippy clean.

## Implementation Notes

- Prefer threading `tenant_id` as an explicit arg into store methods over a hidden ambient — keeps the scoping
  auditable + testable.
- Platform-admin acting via `/tenants/{id}/*` sets the target tenant from the path, guarded by `is_admin`.

## Status Updates

### 2026-07-06 — in progress (weir-api done; weir-app store threading via subagent)

**Approach:** parallelized — a subagent threads `tenant: &str` (first param) through the `weir-app` store
methods + scopes each query by `tenant_id` (+ ripples `tenant` onto `WorkSpec`/`Relay::plan` so enqueued
`work_units` carry the tenant, for [[WEIR-T-0091]]'s claim-by-tenant); I built the `weir-api` side.

**weir-api done (`crates/weir-api/src/lib.rs` + `authz.rs`):**
- `tenant_of(key)` helper — the caller's tenant, implicit from the key (`tenant_id` or `default`). All data
  handlers now take `Extension<AuthenticatedKey>` + pass `tenant_of(&key)` into the store: list/create/fetch/
  remove/run/runs/recent/dead_letters/logs/catalog_list/catalog_import/catalog_unregister/state. Cross-tenant
  reads → `NotFound` → 404 (no existence leak).
- **`/tenants/*` admin surface** (platform-admin): `GET/POST /tenants`, `DELETE /tenants/{id}`,
  `GET/POST /tenants/{id}/keys`, `DELETE /tenants/{id}/keys/{kid}` + `TenantInfo`/`ApiKeyDto` DTOs.
- **authz.rs**: added those 6 routes as `Scope::Platform`/`Admin`. Data routes stay `Scope::Any` — the *role*
  gate; the *tenant* filter is the handler's job (the documented "data scoped by the handler" model). So no new
  `Scope::Tenant` variant needed; reconciles the AC's "move to tenant scope" as handler-level scoping.
- Left `connector_spec`/`discover`/`catalog_available`/`catalog_preview` unscoped (wasm-artifact isolation is
  [[WEIR-T-0092]]).

### 2026-07-06 — done (`2c1f90f`)

Integrated: the subagent threaded `tenant` (first arg) through all `weir-app` scoped methods + queries (chose
to add a `tenant` field to `WorkSpec` that rides `schedules.spec` rather than churn `Relay::plan` — scheduler-
fired runs stay tenant-scoped for free). weir-cli defaults to `DEFAULT_TENANT` on its paths. Full workspace
builds; **weir-api 9 + weir-cli + weir-app 15 + weir-orchestrator all green; clippy clean**.

Added **`cross_tenant_isolation`** (weir-api `tests/api.rs`): two tenant-scoped keys — acme creates a
connection, globex gets **404** on it (not 403) + an empty list, acme sees its own, and a non-admin tenant key
is **denied** the `/tenants` admin surface (403). **Complete.**

Caveat carried to [[WEIR-T-0093]]: the engine write-side still stamps some diagnostics rows (`run_logs`/
`dead_letters` via the engine `Store`) as `tenant_id='default'` — non-default-tenant diagnostics read empty
until engine write-side scoping lands. Runner claim-by-tenant is [[WEIR-T-0091]].
