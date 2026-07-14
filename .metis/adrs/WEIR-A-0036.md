---
id: 001-tenant-isolated-execution-per
level: adr
title: "Tenant-isolated execution — per-tenant runner fleet"
number: 36
short_code: "WEIR-A-0036"
created_at: 2026-07-05T23:56:05.599586+00:00
updated_at: 2026-07-05T23:56:05.599586+00:00
decision_date: 2026-07-05
decision_maker: dylan.storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: true
initiative_id: NULL
---

# ADR-36: Tenant-isolated execution — per-tenant runner fleet

## Context **[REQUIRED]**

[[WEIR-A-0004]] decided **row-level `tenant_id`** tenancy and [[WEIR-I-0017]] shipped the tenant *model*
(`tenant_id` on API keys, the authz `Scope`/`Principal`/`evaluate`) — but nothing is tenant-scoped yet.
[[WEIR-I-0018]] makes tenancy real in depth: a tenant's connectors compile in isolation and its syncs execute
in isolation. Today the control plane is fully untenanted — `connections.name` is a global PK, `work_units`
carry no tenant, and `Executor::claim()` grabs the lowest-id pending unit **across all tenants** (a single
runner processes everyone's work). This ADR pins the isolation architecture so the tasks can proceed.

## Decision **[REQUIRED]**

1. **Physical per-tenant runners.** A runner has a **tenant identity**; `claim()` gains `WHERE tenant_id =
   <runner's tenant>`; a runner only ever leases/executes one tenant's work. A **fleet coordinator** ensures a
   runner per active tenant. In [[WEIR-I-0018]] the fleet is in-process / single-node; [[WEIR-I-0021]] provisions
   + autoscales the same runners on Kubernetes (no runtime redesign there). Mirrors cloacina `tenant_runner_cache`
   + `fleet_coordinator`/`fleet_executor`.
2. **Composite `(tenant_id, name)` scoping key.** `connections` PK becomes `(tenant_id, name)`; `work_units`
   and every scoped table (`run_logs`, `dead_letters`, `stream_state`, `outbox`, `schedules`, `connectors`)
   carry `tenant_id`; refs are `(tenant_id, connection)`. Keeps the human-readable name and lands `tenant_id`
   on `work_units` — exactly what `claim()` needs. Two tenants may both have a `fx-demo`.
3. **Implicit tenant + explicit `/tenants/{id}/*` for admins.** An API key belongs to one tenant; normal routes
   auto-scope to the caller's `tenant_id`. A platform-admin acts on another tenant only via the explicit
   `/tenants/{id}/...` path — deliberate + auditable; no ambient `X-Tenant` header.
4. **Implicit `default` tenant.** Single-tenant deploys use a `default` tenant; the migration backfills existing
   (untenanted) rows to it, so nothing breaks and the common case needs no tenant awareness.
5. **Compile isolation.** `weir-codegen` builds into a per-tenant namespace with `tenant_id` in the artifact
   path + cache key; no cross-tenant artifact reuse can leak a tenant's manifest/config/identity.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **Physical per-tenant runners (chosen)** | Strong isolation (a runner never touches another tenant); maps 1:1 to k8s pod-per-tenant ([[WEIR-I-0021]]); matches cloacina | Pulls the fleet-coordinator/runner-identity work into I-0018; more moving parts | Medium | Higher |
| Logical only (shared runner, tenant-tagged work) | Least code; one worker | A process handles many tenants' data/secrets in memory — weaker isolation; no clean k8s scale story | Higher (isolation) | Lower |
| Schema-per-tenant | DB-enforced isolation | Rejected in [[WEIR-A-0004]] — migrations × N tenants, connection sprawl | High | High |
| Ambient `X-Tenant` for admins | Fewer routes | Ambient cross-tenant context is easy to misconfigure + hard to audit | High | Low |

## Rationale **[REQUIRED]**

Physical per-tenant runners give real isolation (memory, secrets, work) rather than trusting every query to
carry the right filter, and they map directly onto the k8s pod-per-tenant model [[WEIR-I-0021]] will provision —
so the runtime is designed once. The lease/claim model ([[WEIR-A-0010]]/[[WEIR-A-0011]]) already tolerates many
independent claimants; adding a tenant predicate to `claim()` is a small, safe change. Composite
`(tenant_id, name)` keys carry tenancy where it's needed (notably `work_units`) while keeping human-readable
refs. Implicit tenancy + explicit `/tenants/{id}/*` keeps the 99% path trivial and makes cross-tenant action a
deliberate, audited act. The `default` tenant keeps single-tenant deploys zero-friction.

## Consequences **[REQUIRED]**

### Positive
- No cross-tenant read / run / build / secret access; provable with a two-tenant test.
- The runtime built here is exactly what [[WEIR-I-0021]] scales on k8s — one design, not two.
- Single-tenant deploys are unaffected (implicit `default`).

### Negative
- A schema migration recomposes `connections`' PK + adds `tenant_id` across scoped tables + FKs (ORM churn).
- A fleet coordinator + runner identity is net-new machinery (idle-tenant reaping, fairness across tenants).
- Per-tenant compile artifacts cost cache space (no cross-tenant sharing).

### Neutral
- Fairness/quotas across tenants (one tenant starving others) is explicitly deferred (noted in [[WEIR-I-0018]]).

## Review Schedule **[CONDITIONAL: Temporary Decision]**

Permanent architectural decision; revisit only if the runner-per-tenant model proves too heavy at high tenant
counts (then reconsider a bounded shared pool keyed by tenant per work-unit).
