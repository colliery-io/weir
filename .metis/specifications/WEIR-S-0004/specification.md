---
id: sync-engine-orchestrator
level: specification
title: "Sync Engine (Orchestrator)"
short_code: "WEIR-S-0004"
created_at: 2026-06-17T02:06:09.055042+00:00
updated_at: 2026-06-17T02:06:09.055042+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Sync Engine (Orchestrator)

*Component spec under [[WEIR-I-0001]]. Source: component PRD §3.*

## Overview **[REQUIRED]**

Owns *when, in what order, how many at once, what to do on failure, and what state carries forward* for every sync — decoupled from the connector code that moves bytes. Connectors stay simple; reliability, scheduling, concurrency, and checkpointing are solved once, centrally (the cloacina pattern). It is sealed from the execution-model decision by the dispatch seam to the Runtime ([[WEIR-S-0005]]).

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Control Plane ([[WEIR-S-0002]])**: triggers, cancels, queries runs.

### External Systems
- **Connector Runtime ([[WEIR-S-0005]])**: receives dispatched work units behind the dispatch seam.
- **Metadata & State Store ([[WEIR-S-0009]])**: durable config, state, checkpoints, run history.
- **Secrets Manager ([[WEIR-S-0010]])**: secret resolution path (ADR-0013).

### Boundaries
Inside: scheduling, planning, dispatch, retries, checkpoint commit. Outside: moving bytes (Runtime), execution/isolation primitive (Runtime), persistence engine (Store).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-SE-1 | Resolve schedules; accept manual/API triggers; run backfills and ranged re-syncs. | Scheduling (capabilities §G). |
| REQ-SE-2 | Plan runs: load config + last state, decompose into work units along declared partitioning. | Partitioned/parallel reads (§A). |
| REQ-SE-3 | Dispatch work units to the Runtime under global and per-connection concurrency limits. | Concurrency control (§H). |
| REQ-SE-4 | Apply retry/backoff on transient failure; enforce cancellation and timeouts. | Reliability (§G/§H). |
| REQ-SE-5 | Commit checkpoints transactionally; resume from the last durable point. | Resumable checkpointing (§A). |
| REQ-SE-6 | Emit run-lifecycle events. | Observability/lineage (§I). |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-SE-1 | Crash-safe: no checkpoint advances past work that did not durably complete. | Correctness. |
| NFR-SE-2 | Horizontal scalability toward thousands of concurrent connections. | Scale. |
| NFR-SE-3 | At-least-once dispatch with idempotent checkpointing. | Delivery semantics (ADR-0011). |
| NFR-SE-4 | Fairness and isolation across tenants. | Multi-tenancy. |
| NFR-SE-5 | Single-binary deployable; no mandatory broker at small scale. | "Open core stands alone." |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Durable state store
- **Context**: PostgreSQL multi-node / SQLite single-node behind one access interface. **ADR**: WEIR-A-0009.

### Decision Area: Work distribution
- **Context**: DB-backed queue (Postgres SKIP LOCKED) vs. message broker, behind a broker-shaped dispatch seam. **ADR**: WEIR-A-0010.

### Decision Area: Delivery & checkpoint semantics
- **Context**: At-least-once + idempotent checkpoints vs. exactly-once attempts. **ADR**: WEIR-A-0011.

### Decision Area: Partition/slice planning ownership
- **Context**: Connector declares partitionability; Engine plans the slices. **ADR**: WEIR-A-0012.

### Decision Area: Secret resolution path
- **Context**: Engine resolves and passes credentials vs. Runtime redeems a handle directly. **ADR**: WEIR-A-0013.

## Implementation status (WEIR-I-0004, 2026-06-20)

The engine now plans + dispatches against the declared contract dimensions, with live Postgres conformance tests via `weir-connector-postgres`:
- **REQ-SE-2 (partitioned/parallel reads)** — a source declares `PartitionScheme`; the relay fans out one work unit per `Partition` (`Relay::plan_partitioned`), run in parallel with **independent per-partition checkpoints** (per-partition `state_key`). Realizes [[WEIR-A-0012]] (connector declares, engine plans). — [[WEIR-T-0027]].
- **REQ-SE-5 (resumable checkpointing)** — generalized across `SyncMode::{FullRefresh, Incremental}` ([[WEIR-T-0026]]) and `Cdc` (WAL LSN/slot in `StreamState.opaque`, [[WEIR-T-0028]]), plus `WriteMode` honoring ([[WEIR-T-0025]]). The engine commits `cursor` + `opaque` per chunk.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0009 | Durable state store | proposed | Postgres/SQLite behind one interface. |
| WEIR-A-0010 | Work distribution | proposed | DB-queue vs. broker dispatch seam. |
| WEIR-A-0011 | Delivery & checkpoint semantics | proposed | At-least-once + idempotent checkpoints. |
| WEIR-A-0012 | Partition planning ownership | proposed | Connector declares, Engine plans. |
| WEIR-A-0013 | Secret resolution path | proposed | Engine-passes vs. Runtime-redeems. |
