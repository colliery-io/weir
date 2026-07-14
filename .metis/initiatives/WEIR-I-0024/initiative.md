---
id: holistic-ops-views-super-operator
level: initiative
title: "Holistic ops views — super-operator + per-tenant system health"
short_code: "WEIR-I-0024"
created_at: 2026-07-07T04:00:25.482163+00:00
updated_at: 2026-07-07T11:12:43.170698+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: holistic-ops-views-super-operator
---

# Holistic ops views — super-operator + per-tenant system health

## Context

The UI has an operations view + a tenant switcher ([[WEIR-I-0019]]), and the platform emits rich signals —
Prometheus `/metrics`, OpenLineage, per-run rows/duration, dead letters ([[WEIR-I-0020]]). But there's no
**holistic health view**: an operator can't see, at a glance, "is everything healthy, and if not, where?" This
initiative builds two such views.

## Goals

1. **Super-operator (cross-tenant) view** — the platform admin sees the whole system: per-tenant health rollups
   (runs succeeding/failing, queue depth, dead letters, throughput), which tenants/connections need attention,
   and runner/fleet status. The "is the platform OK?" screen.
2. **Per-tenant view** — a tenant sees *their* systems holistically: each connection's freshness (last successful
   run + lag), failure/retry state, rows moved over time, dead-letter counts, and lineage at a glance. The "are
   my pipelines healthy?" screen.

Both should read at a glance (summary before detail; state encoded as colour/severity, not just numbers) and
respect tenancy — the tenant view is scoped; the super-operator view is platform-admin-only.

## Non-Goals

- Grafana/external dashboards (we expose `/metrics` for those; this is the *in-product* view).
- Alerting/paging (a later ops-maturity step).

## Design decisions (2026-07-07, approved)

- **Data source**: **store-derived aggregate endpoints** — new tenant-scoped API endpoints over
  `runs`/`work_units`/`dead_letters`/`stream_state`. Authoritative, row-exact, tenancy for free. `/metrics` stays
  for external Grafana.
- **Scope**: **both views in v1** — they share the aggregate endpoints, so the super-operator rollup is mostly
  extra UI over the same data.
- **Health signal**: a connection is **green/amber/red** on **freshness** (last success within schedule + grace),
  **error-rate** (recent failures over a threshold), and **dead-letters** (climbing). Rolled up per tenant.
  Sensible fixed thresholds in v1; per-connection config is a later pass.
- **Shape**: a **health dashboard** (status grid + sparklines + a "needs attention" list), not another list. The
  tenant view keys on **connections**; the super-operator view keys on **tenants** (rollup) → drill to connections.
- No new ADR — sits under [[WEIR-A-0036]] (tenancy) + [[WEIR-A-0022]] (observability).

## Proposed decomposition (for sign-off)

- **T-a — Aggregate health endpoints:** the health computation (freshness/error-rate/dead-letter → green/amber/red)
  + tenant-scoped endpoints for per-connection health and a **platform-admin** cross-tenant rollup + fleet status.
  Unit-tested health logic.
- **T-b — Per-tenant health dashboard (UI):** connection status grid (colour by health), per-connection detail
  (last run, lag, throughput sparkline, dead letters), "needs attention" surfacing; + its e2e.
- **T-c — Super-operator cross-tenant view (UI):** per-tenant health cards + what-needs-attention across all
  tenants + runner/fleet status, drill into a tenant → its connections; platform-admin-gated; + its e2e.

## Exit Criteria (draft — refine in design)

- [ ] A per-tenant health view: per-connection freshness/lag, run success/failure, dead letters, throughput.
- [ ] A super-operator cross-tenant view: per-tenant health rollups + what needs attention + fleet status.
- [ ] Backed by aggregate endpoints (tenant-scoped; super-operator gated to platform admin).
- [ ] e2e coverage; workspace + clippy + UI e2e green.
