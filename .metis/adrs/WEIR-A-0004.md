---
id: 001-multi-tenancy-model
level: adr
title: "Multi-tenancy model"
number: 1
short_code: "WEIR-A-0004"
created_at: 2026-06-17T02:11:51.152973+00:00
updated_at: 2026-06-17T02:11:51.152973+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0004: Multi-tenancy model

**Status:** Decided (2026-07-05, via [[WEIR-A-0008]]). *Raised by: [[WEIR-S-0009]] Metadata & State Store, [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0010]] Secrets Manager.*

## Context **[REQUIRED]**

Multi-tenancy is periphery (capabilities §J), but the open core must provide the isolation *primitives* it needs. We must decide the tenant boundary and isolation mechanism so the core data model, engine fairness, and secret scoping are tenant-aware from the start. Settled when weir adopted cloacina's auth model ([[WEIR-A-0008]]), which is tenant-aware by row.

## Decision **[REQUIRED]**

**Decided: row-level `tenant_id` scoping enforced through the authz seam** ([[WEIR-A-0008]], mirroring
cloacina) — every tenant-scoped resource carries a `tenant_id` column, and access is gated by the ABAC
`evaluate()` (`Scope::Tenant` + `Principal.tenant`). This **supersedes** the earlier schema-per-tenant
proposal below: row-level avoids schema sprawl, matches the proven cloacina control plane, and lets a single
migration/DDL serve all tenants. Engine fairness + secret scoping key off the same `tenant_id`.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Schema-per-tenant | Strong data isolation; clean backup/restore per tenant | Schema sprawl at scale | Medium | Medium |
| Row-level (tenant_id) | Simple; one schema | Weaker isolation; every query must filter | Medium | Low |
| Database-per-tenant | Strongest isolation | Operational heaviness | Low | High |

## Rationale **[REQUIRED]**

Schema-per-tenant balances isolation against operational weight and aligns with the cloacina-style orchestration patterns. The core only needs to expose the primitives; the managed multi-tenant control plane is periphery.

## Consequences **[REQUIRED]**

### Positive
- Tenant isolation designed in, not retrofitted.

### Negative
- Schema management complexity grows with tenant count.

### Neutral
- Couples to the data model ([[WEIR-A-0007]]) and state store ([[WEIR-A-0009]]).
