---
id: tenant-isolation-compile-execution
level: initiative
title: "Tenant isolation — compile + execution"
short_code: "WEIR-I-0018"
created_at: 2026-07-05T23:44:20.109443+00:00
updated_at: 2026-07-06T01:12:09.122589+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: tenant-isolation-compile-execution
---

# Tenant isolation — compile + execution Initiative

## Context

[[WEIR-A-0004]] (decided) + [[WEIR-A-0008]] gave weir a **tenant model** — `tenant_id` on API keys, the authz
`Scope`/`Principal`/`evaluate`, row-level scoping — but **no resource today is tenant-scoped**: every route is
`Scope::Any`, connections/runs/catalog carry no `tenant_id`, connector **builds** and **executions** are
global, and secrets aren't tenant-scoped. This initiative makes tenancy real **in depth**: a tenant's
connectors compile in isolation and its syncs execute in isolation, so tenant A can never read, run, or
interfere with tenant B's data, connectors, or work.

## Goals & Non-Goals

**Goals**
- **Tenant CRUD** — the `tenants` table exists ([[WEIR-T-0083]]); add create/list/delete (platform-admin,
  the `Scope::Platform` slot already reserved) + `/tenants/{id}/keys` management (mirror cloacina `/tenants/*`).
- **Tenant-scoped data** — connections, work_units, runs, dead-letters, stream_state, catalog entries carry a
  `tenant_id`; the store + handlers filter by the caller's tenant; the authz table moves the relevant routes to
  tenant scope, and `evaluate`'s `Tenant` branch is exercised for real.
- **Execution isolation** — the orchestrator/engine key work + leases + state by `tenant_id`; a runner only
  ever sees one tenant's work. Mirror cloacina `tenant_runner_cache` + `fleet_coordinator`/`fleet_executor`
  (a per-tenant runner/fleet model). Secrets ([[WEIR-A-0013]]/[[WEIR-A-0033]]) scope to the tenant.
- **Compile isolation** — connector onboarding/build (`weir-codegen`, wasm build) is per-tenant: a tenant's
  onboarded manifests/crates + built artifacts live in its namespace; no cross-tenant artifact reuse leaks
  identity/config. Cache keys include `tenant_id`.
- **Isolation tests** — a two-tenant integration test proving no cross-tenant read/run/build/secret access.

**Non-Goals**
- The tenant-aware **UI** (that's [[WEIR-I-0019]]) and **k8s** runner provisioning (that's [[WEIR-I-0021]]).
- Enterprise SSO / group→tenant mapping (periphery, [[WEIR-A-0005]]).
- Per-tenant resource quotas / billing (a later concern).

## Prior art (cloacina)

`~/Desktop/cloacina/crates/cloacina-server/src/`: `tenant_runner_cache.rs`, `fleet_coordinator.rs`,
`fleet_executor.rs` (per-tenant runner fleet), `routes/authz.rs` (`Scope::TenantParam` + `/tenants/{tenant_id}/*`),
`identity.rs` (tenant on the principal). weir already mirrors the authz + key model ([[WEIR-I-0017]]).

## Weir surfaces to change

- `crates/weir-schema` — add `tenant_id` to `connections`/`work_units`/`run_logs`/`dead_letters`/`stream_state`/
  `connectors` (a migration); tenant CRUD store.
- `crates/weir-app` — tenant-scope every store method by the caller's `tenant_id`; tenant CRUD.
- `crates/weir-api` — `/tenants/*` routes + move connection/catalog routes to tenant scope in `authz.rs`.
- `crates/weir-orchestrator` / `crates/weir-engine` — key work/leases/state by tenant; a runner sees one tenant.
- `crates/weir-codegen` — per-tenant build namespace + cache keys.

## Design decisions (2026-07-05, approved)

1. **Execution isolation = physical per-tenant runners (now).** `claim()` gains a `WHERE tenant_id = <runner's
   tenant>` filter; a runner has a **tenant identity** and only ever sees one tenant's work. A **fleet
   coordinator** ensures a runner per active tenant (in-process / single-node in this initiative). Mirror
   cloacina `tenant_runner_cache` + `fleet_coordinator`/`fleet_executor`. → **This pulls the runner-fleet model
   into I-0018**; [[WEIR-I-0021]] is now purely *k8s provisioning + autoscaling of these runners* (no runtime
   redesign there).
2. **Scoping key = composite `(tenant_id, name)`.** `connections` PK becomes `(tenant_id, name)`; `work_units`
   and every scoped table carry `tenant_id`; refs are `(tenant_id, connection)`. Keeps the human-readable name
   and naturally lands `tenant_id` on `work_units` (what `claim()` needs). Two tenants can both have `fx-demo`.
3. **Cross-tenant admin = implicit tenant + explicit `/tenants/{id}/*`.** A key belongs to one tenant; normal
   routes auto-scope to it. A platform-admin acts on another tenant only via the explicit `/tenants/{id}/...`
   path (mirrors cloacina) — deliberate + auditable, no ambient `X-Tenant`.

These warrant an ADR — **[[WEIR-A-0036]] Tenant-isolated execution (per-tenant runner fleet)** — decided as the
gate out of design.

## Remaining sub-questions (settle during tasks)

- Default tenant: single-tenant deploys use an implicit `default` tenant; existing (untenanted) rows backfill
  to it in the migration (so nothing breaks). *(Leaning yes — confirm in the schema task.)*
- Does the fleet coordinator spin runners eagerly (per active tenant) or lazily (on first pending work)?
- Compile cache: `tenant_id` in the artifact path + cache key — is any artifact safely shareable across
  tenants (e.g. a public manifest with no secrets), or is all per-tenant for simplicity?

## Exit Criteria (draft — refine in design)

- [ ] `tenants` CRUD (platform-admin) + `/tenants/{id}/keys`; existing single-tenant deploys keep working via a `default` tenant.
- [ ] Connections/runs/catalog/secrets are tenant-scoped; a caller only ever sees/acts on its tenant's resources.
- [ ] Execution is tenant-isolated (work/leases/state keyed by tenant; a runner sees one tenant).
- [ ] Connector compile/build is tenant-namespaced (no cross-tenant artifact/identity leak).
- [ ] A two-tenant integration test proves no cross-tenant read/run/build/secret access; workspace + clippy green.
