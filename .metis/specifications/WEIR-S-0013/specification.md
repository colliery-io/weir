---
id: integration-adapters
level: specification
title: "Integration Adapters"
short_code: "WEIR-S-0013"
created_at: 2026-06-17T02:06:38.886931+00:00
updated_at: 2026-06-17T02:06:38.886931+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Integration Adapters

*Component spec under [[WEIR-I-0001]]. Source: component PRD §12.*

## Overview **[REQUIRED]**

Thin adapters that make the surrounding stack treat weir as a first-class citizen — Airflow provider, DataHub (via OpenLineage), Superset, Terraform. They are adoption drivers, sit on top of the Control Plane API ([[WEIR-S-0002]]) and the lineage stream ([[WEIR-S-0011]]), and never reach into internals.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Data platform engineer**: orchestrates syncs from Airflow, manages resources via Terraform.

### External Systems
- **Control Plane API ([[WEIR-S-0002]])**: the only coupling point (API-only).
- **Observability & Lineage ([[WEIR-S-0011]])**: OpenLineage stream consumed by DataHub.
- **Airflow, DataHub, Superset, Terraform**: the integrated tools.

### Boundaries
Inside: thin adapters/providers, versioned against the API. Outside: platform internals (never touched), managed/multi-tenant polish (vendor).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-IA-1 | Airflow provider/operator to orchestrate syncs as tasks. | Integration differentiator (§L). |
| REQ-IA-2 | DataHub integration consuming the OpenLineage stream. | Metadata-plane visibility (§L). |
| REQ-IA-3 | Superset-facing exposure of sync and freshness metadata. | Downstream dashboards (§L). |
| REQ-IA-4 | Terraform provider for IaC management of platform resources. | Parity (§L). |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-IA-1 | API-only coupling — no privileged access to internals. | Architectural invariant. |
| NFR-IA-2 | Versioned against the Control Plane API contract. | Stability. |
| NFR-IA-3 | Independently releasable from the core. | Release cadence. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Integration adapter strategy & packaging
- **Context**: Airflow provider packaging, DataHub via OpenLineage, Superset exposure, Terraform provider; independent release cadence. **ADR**: WEIR-A-0025.

### Decision Area: Lineage standard (shared) / API contract (shared)
- **ADR**: WEIR-A-0022, WEIR-A-0006.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0025 | Integration adapter strategy | proposed | Packaging + independent release cadence. |
| WEIR-A-0022 | Observability & lineage standard | proposed | Shared lineage stream. |
| WEIR-A-0006 | API protocol & versioning | proposed | Shared API contract. |
