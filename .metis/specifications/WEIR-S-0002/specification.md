---
id: control-plane-api
level: specification
title: "Control Plane / API"
short_code: "WEIR-S-0002"
created_at: 2026-06-17T02:04:37.236960+00:00
updated_at: 2026-06-17T02:04:37.236960+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Control Plane / API

*Component spec under [[WEIR-I-0001]]. Source: component PRD §1. The decisions it forces are tracked as ADRs.*

## Overview **[REQUIRED]**

The system's front door and the single source of truth for configuration. It exists so every surface — UI, integrations, automation — speaks to one consistent, versioned API, and so configuration has one authoritative owner instead of being smeared across components. It mediates all reads and writes of sources, destinations, connections, and schedules, and triggers runs by handing off to the Sync Engine ([[WEIR-S-0004]]). **It never executes data movement itself.**

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Platform operator / engineer**: configures and operates pipelines via the API.
- **Web UI ([[WEIR-S-0003]])**: a pure client of this API; no privileged backdoor.
- **Integration adapters ([[WEIR-S-0013]])**: Airflow/Terraform/etc., API-only clients.

### External Systems
- **Sync Engine ([[WEIR-S-0004]])**: receives run trigger/cancel/query hand-offs.
- **Connector Catalog ([[WEIR-S-0007]])**: browse/install operations proxied here.
- **Metadata & State Store ([[WEIR-S-0009]])**: all persistent config state lives here.

### Boundaries
Inside: configuration CRUD, validation, run-lifecycle API, audit emission. Outside: data movement (Runtime), scheduling internals (Engine), persistence-engine choice (Store).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-CP-1 | CRUD for workspaces, sources, destinations, connections, schedules. | Single authoritative config owner. |
| REQ-CP-2 | Stream/column selection and connection-level mapping configuration. | Connection definition (capabilities §D). |
| REQ-CP-3 | Test-connection, validation, and dry-run endpoints. | UX differentiator (capabilities §D). |
| REQ-CP-4 | Trigger, cancel, and query sync runs, delegating execution to the Sync Engine. | Control plane never executes. |
| REQ-CP-5 | Proxy connector browse/install operations to the Catalog. | One API surface for users. |
| REQ-CP-6 | Serve a complete programmatic API; UI and adapters are pure clients. | UI ≡ API client invariant. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-CP-1 | Backward-compatible API within a major version. | Stable contract for clients/adapters. |
| NFR-CP-2 | AuthN on every endpoint; authorization hooks present even when RBAC is periphery. | Open-core seam for RBAC/SSO. |
| NFR-CP-3 | Stateless instances (all state in the Store) for horizontal scaling. | Scale-out. |
| NFR-CP-4 | Low-latency config reads; safe concurrent edits via optimistic concurrency. | Correctness under concurrent edits. |
| NFR-CP-5 | Every mutation emits an audit event. | Governance/audit (periphery consumes). |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: API protocol & versioning
- **Context**: REST vs. gRPC vs. both; the backward-compatibility/versioning policy.
- **ADR**: WEIR-A-0006.

### Decision Area: Domain/config data model
- **Context**: Core entities (workspace, source, destination, connection, stream selection, schedule, mapping) and relationships.
- **ADR**: WEIR-A-0007.

### Decision Area: Auth baseline & RBAC seam
- **Context**: AuthN in the open core and the hook surface where RBAC/SSO (periphery) attaches.
- **ADR**: WEIR-A-0008.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0006 | API protocol & versioning | proposed | REST/gRPC choice + versioning policy. |
| WEIR-A-0007 | Domain/config data model | proposed | Core entities and relationships. |
| WEIR-A-0008 | Auth baseline & RBAC seam | proposed | Open-core AuthN + periphery RBAC hook. |
