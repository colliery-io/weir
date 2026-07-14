---
id: observability-lineage
level: specification
title: "Observability & Lineage"
short_code: "WEIR-S-0011"
created_at: 2026-06-17T02:06:33.900276+00:00
updated_at: 2026-06-17T02:06:33.900276+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Observability & Lineage

*Component spec under [[WEIR-I-0001]]. Source: component PRD §10.*

## Overview **[REQUIRED]**

Collects run metrics, logs, and status, and emits lineage so operators can see health and the metadata plane (DataHub) gets first-class visibility. It exists so observability is a designed capability rather than scattered logging, and so lineage rides an open standard (OpenLineage) rather than a vendor-specific hook.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Operator**: views health, history, freshness; receives alerts.

### External Systems
- **Sync Engine ([[WEIR-S-0004]])**: emits run-lifecycle events.
- **Integration Adapters ([[WEIR-S-0013]])** / **DataHub**: consume the OpenLineage stream.
- **Control Plane / UI ([[WEIR-S-0002]]/[[WEIR-S-0003]])**: surface metrics and status.

### Boundaries
Inside: metric/log/status collection, OpenLineage emission, freshness/SLA, alerting baseline. Outside: dashboards/BI (Superset's job), catalog/governance platform (DataHub's job).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-OB-1 | Collect run status, per-stream progress, metrics (rows/bytes/latency), and logs. | Observability (§I). |
| REQ-OB-2 | Emit OpenLineage events per sync (datasets, run, job). | Integration anchor (§I/§L). |
| REQ-OB-3 | Track freshness/SLA per connection. | Freshness (§I). |
| REQ-OB-4 | Provide failure alerting (webhook/email baseline). | Operability. |
| REQ-OB-5 | Expose this data to the Control Plane/UI and integration adapters. | Surfacing. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-OB-1 | Emission is async and never blocks or slows the run path. | Throughput. |
| NFR-OB-2 | Standards-based (OpenLineage) for portability. | No vendor lock-in. |
| NFR-OB-3 | Bounded overhead and storage with a retention policy. | Cost/scale. |
| NFR-OB-4 | Pluggable sinks (DataHub and others). | Vendor-neutral. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Observability & lineage standard & transport
- **Context**: OpenLineage adoption, metrics/log transport, event schema, pluggable sinks. **ADR**: WEIR-A-0022.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0022 | Observability & lineage standard | proposed | OpenLineage + transport + pluggable sinks. |
