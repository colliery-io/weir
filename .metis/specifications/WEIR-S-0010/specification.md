---
id: secrets-manager
level: specification
title: "Secrets Manager"
short_code: "WEIR-S-0010"
created_at: 2026-06-17T02:06:32.212799+00:00
updated_at: 2026-06-17T02:06:32.212799+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Secrets Manager

*Component spec under [[WEIR-I-0001]]. Source: component PRD §9.*

## Overview **[REQUIRED]**

Stores connection credentials and injects them into connector runs without leaking plaintext into logs, config, or — ideally — the Engine. It exists to contain the most sensitive data behind a narrow, pluggable interface so enterprises can back it with their own vault or KMS.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Connection config (via Control Plane [[WEIR-S-0002]])**: stores credentials.

### External Systems
- **Connector Runtime ([[WEIR-S-0005]])**: redeems short-lived handles at run time.
- **Sync Engine ([[WEIR-S-0004]])**: participates in the secret resolution path (ADR-0013).
- **Backends**: env/file (dev); Vault / KMS / cloud secret managers (prod).

### Boundaries
Inside: credential store/retrieve/rotate/revoke, handle issuance, backend abstraction. Outside: credential *use* (Runtime), encryption primitives (delegated to backend).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-SM-1 | Store and retrieve credentials, scoped per tenant and connection. | Tenant isolation (§J). |
| REQ-SM-2 | Issue short-lived secret handles redeemed at run time (per ADR-0013 lean path). | Minimize blast radius. |
| REQ-SM-3 | Support pluggable backends (env/file dev; Vault/KMS/cloud prod). | Enterprise fit. |
| REQ-SM-4 | Rotate and revoke credentials. | Operability/security. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-SM-1 | Plaintext never logged, never placed in URLs or config; minimized in memory and blast radius. | Core security guarantee. |
| NFR-SM-2 | Encryption at rest (delegated to backend) and in transit. | Confidentiality. |
| NFR-SM-3 | Auditable secret access. | Governance. |
| NFR-SM-4 | Backend pluggability without changing callers. | Abstraction stability. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Secrets backend abstraction
- **Context**: Pluggable backends (env/file dev; Vault/KMS/cloud prod) behind one interface. **ADR**: WEIR-A-0021.

### Decision Area: Secret resolution path (shared)
- **ADR**: WEIR-A-0013.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0021 | Secrets backend abstraction | proposed | Pluggable backends behind one interface. |
| WEIR-A-0013 | Secret resolution path | proposed | Runtime redeems handle directly. |
