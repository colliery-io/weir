---
id: connector-runtime-worker
level: specification
title: "Connector Runtime (Worker)"
short_code: "WEIR-S-0005"
created_at: 2026-06-17T02:06:17.105299+00:00
updated_at: 2026-06-17T02:06:17.105299+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Connector Runtime (Worker)

*Component spec under [[WEIR-I-0001]]. Source: component PRD §4. Resolves the headline open decision (execution/isolation model, WEIR-A-0002) behind the Engine's dispatch seam.*

## Overview **[REQUIRED]**

Executes connector instances and moves the data — the only component that touches external source and destination data. It implements the execution/isolation model and therefore resolves the headline open decision, behind the Engine's dispatch seam ([[WEIR-S-0004]]). It exists to contain the riskiest concern — running third-party code that touches data and secrets — in one isolated, swappable place.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Sync Engine ([[WEIR-S-0004]])**: dispatches work units; receives records and checkpoint deltas.

### External Systems
- **Connector Catalog ([[WEIR-S-0007]])**: source of versioned connector artifacts.
- **Secrets Manager ([[WEIR-S-0010]])**: redeems secret handles for credentials.
- **External sources/destinations**: the actual data systems (APIs, DBs, warehouses, SaaS).

### Boundaries
Inside: connector instantiation, extract/load/reverse-ETL execution, isolation, resource limits. Outside: scheduling/retry policy (Engine), credential storage (Secrets Manager), contract definition (SDK).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-RT-1 | Load a connector artifact from the Catalog and instantiate it for a work unit. | Dispatch-time fetch. |
| REQ-RT-2 | Execute extract, load, and reverse-ETL writes per the connector contract. | Core data movement (§A/§B/§C). |
| REQ-RT-3 | Stream records and checkpoint deltas back to the Engine mid-flight. | Resumable checkpointing. |
| REQ-RT-4 | Redeem a secret handle and inject credentials into the run. | Secret resolution (ADR-0013). |
| REQ-RT-5 | Enforce per-run resource limits and isolation; report structured errors. | Safe untrusted execution (§H). |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-RT-1 | Isolation: untrusted connector code cannot crash the host or read another tenant's secrets. | Multi-tenant safety. |
| NFR-RT-2 | Lean footprint: per-connector overhead negligible against data-transfer time (no OS-image tax). | Differentiator vs. container-per-connector. |
| NFR-RT-3 | Throughput bound by source/destination I/O, not runtime overhead. | Performance. |
| NFR-RT-4 | Fast cold start (sub-second native; bounded for sandboxed paths). | Responsiveness/scaling. |
| NFR-RT-5 | Backpressure-aware to honor destination rate limits. | Reverse-ETL safety (§C). |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Execution & isolation model (HEADLINE)
- **Context**: In-process ABI vs. WASM component vs. sandboxed subprocess vs. container; where the per-connector isolation boundary sits for untrusted, multi-tenant code.
- **Constraints**: leanness↔isolation move in opposite directions; container demoted to compatibility escape hatch.
- **ADR**: WEIR-A-0002.

### Decision Area: Packaging/distribution format
- **Context**: cdylib / WASM component / OCI image / declarative manifest, coupled to WEIR-A-0002. **ADR**: WEIR-A-0015.

### Decision Area: Secret resolution path / Language split
- **ADR**: WEIR-A-0013, WEIR-A-0001.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0002 | Connector execution & isolation model | proposed | The headline open decision. |
| WEIR-A-0015 | Packaging & distribution format | proposed | Artifact format, coupled to ADR-0002. |
| WEIR-A-0013 | Secret resolution path | proposed | Runtime redeems handle directly. |
| WEIR-A-0001 | Core language split | proposed | Rust core, Python SDK. |
