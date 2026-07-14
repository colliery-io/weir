---
id: metadata-state-store
level: specification
title: "Metadata & State Store"
short_code: "WEIR-S-0009"
created_at: 2026-06-17T02:06:29.770417+00:00
updated_at: 2026-06-17T02:06:29.770417+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Metadata & State Store

*Component spec under [[WEIR-I-0001]]. Source: component PRD §8.*

## Overview **[REQUIRED]**

The persistence layer for config, catalog metadata, sync state/checkpoints, and run history. It exists to give the stateless services one transactional source of truth and to make crash-safe checkpointing possible. It defines the data domains and the access interface; the database engine is a backing-service decision made inside it.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Control Plane ([[WEIR-S-0002]])** and **Sync Engine ([[WEIR-S-0004]])**: stateless consumers of the typed access interface.

### External Systems
- **PostgreSQL / SQLite**: backing engines (multi-node / single-node).

### Boundaries
Inside: data domains, typed access interface, tenant scoping, migrations, backup/restore. Outside: business logic (services), credential plaintext (Secrets Manager).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-ST-1 | Persist config domains (workspaces, sources, destinations, connections, schedules). | Source of truth. |
| REQ-ST-2 | Persist sync state and per-stream checkpoints transactionally with run state. | Crash-safe checkpointing. |
| REQ-ST-3 | Persist run history and connector-catalog metadata. | Observability + catalog. |
| REQ-ST-4 | Provide a typed access interface and enforce tenant scoping. | Multi-tenant isolation. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-ST-1 | Transactional consistency for combined checkpoint + run-state updates. | Correctness (engine NFR-SE-1). |
| NFR-ST-2 | Deployment flexibility: embedded SQLite single-node, PostgreSQL multi-node. | "Open core stands alone." |
| NFR-ST-3 | Multi-tenant isolation (schema-per-tenant pattern). | Tenancy (ADR-0004). |
| NFR-ST-4 | Backup/restore and schema-evolution (migration) support. | Operability. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Postgres/SQLite split
- **Context**: Multi-node Postgres / single-node SQLite behind one access interface. **ADR**: WEIR-A-0009.

### Decision Area: Data model (shared) / Multi-tenancy model
- **ADR**: WEIR-A-0007, WEIR-A-0004.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0009 | Durable state store | proposed | Postgres/SQLite split behind one interface. |
| WEIR-A-0007 | Domain/config data model | proposed | Shared core entities. |
| WEIR-A-0004 | Multi-tenancy model | proposed | Schema-per-tenant vs. row-level. |
