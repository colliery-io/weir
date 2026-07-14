---
id: web-ui
level: specification
title: "Web UI"
short_code: "WEIR-S-0003"
created_at: 2026-06-17T02:06:01.032199+00:00
updated_at: 2026-06-17T02:06:01.032199+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Web UI

*Component spec under [[WEIR-I-0001]]. Source: component PRD §2.*

## Overview **[REQUIRED]**

The no-code surface, so non-engineers can configure and monitor pipelines without touching the API. It is a strict client of the Control Plane API ([[WEIR-S-0002]]) with no privileged backdoor. We take parity on capability but freedom on flows — the friction-paving happens here.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Non-engineer operator**: configures and monitors pipelines via the UI.
- **Engineer**: uses the connector builder and migration flow.

### External Systems
- **Control Plane API ([[WEIR-S-0002]])**: the only backend the UI talks to.

### Boundaries
Inside: configuration, monitoring, builder launch, migration flow. Outside: anything the API does not expose (UI ≡ API client).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-UI-1 | Configure sources, destinations, connections (stream/column selection, sync mode, schedule, mapping). | No-code surface (capabilities §M). |
| REQ-UI-2 | Browse and install connectors; launch the connector builder. | Catalog + builder access. |
| REQ-UI-3 | Monitor runs: status, history, per-stream progress, logs, freshness. | Observability surface. |
| REQ-UI-4 | Surface validation and test-connection results inline. | UX differentiator. |
| REQ-UI-5 | Drive the Airbyte migration flow. | Adoption lever (capabilities §F/§M). |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-UI-1 | Accessibility to WCAG AA. | Inclusive, enterprise-acceptable. |
| NFR-UI-2 | Handles the API's async nature (long runs, streaming status) without blocking UX. | Long-running syncs. |
| NFR-UI-3 | Exposes nothing the API doesn't (UI ≡ API client). | Architectural invariant. |
| NFR-UI-4 | Internationalization-ready. | Community/global reach. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: UI architecture & stack
- **Context**: SPA vs. server-driven; framework/stack; strictly-API-client posture.
- **ADR**: WEIR-A-0024.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0024 | Web UI architecture | proposed | SPA vs. server-driven; stack; API-client posture. |
