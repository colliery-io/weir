---
id: deployment-operator
level: specification
title: "Deployment & Operator"
short_code: "WEIR-S-0012"
created_at: 2026-06-17T02:06:37.074451+00:00
updated_at: 2026-06-17T02:06:37.074451+00:00
parent: WEIR-I-0001
blocked_by: []
archived: false

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Deployment & Operator

*Component spec under [[WEIR-I-0001]]. Source: component PRD §11. The clean core/periphery split lives here.*

## Overview **[REQUIRED]**

Packages the platform for self-hosting: a simple single-node path (core) and a production Kubernetes operator with autoscaling (periphery). It exists to make "the open core stands alone" literally true — one-command single-node — while giving a vendor a clear operational moat in the operator and autoscaler.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Self-hoster (core)**: brings up single-node or basic K8s.
- **Vendor / platform operator (periphery)**: runs the Helm operator + autoscaler.

### External Systems
- **Sync Engine ([[WEIR-S-0004]])**: provides the queue-depth/load signal that drives autoscaling (ADR-0010 coupling).
- **Kubernetes**: target platform for charts/operator.

### Boundaries
Inside: single-node + basic K8s (core); operator + autoscaler (periphery); upgrade/config management. Outside: managed multi-tenant control plane (vendor), the open-core/periphery *policy* boundary (ADR-0005).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-DP-1 | Single-node deployment (one binary / compose) — core. | "Open core stands alone" (§K). |
| REQ-DP-2 | Basic Kubernetes deploy (chart/manifests) — core. | Open baseline (§K). |
| REQ-DP-3 | Helm operator for lifecycle, upgrades, and scaling — periphery. | Vendor operational moat. |
| REQ-DP-4 | Worker autoscaling driven by Engine queue depth / load — basic open, advanced periphery. | Headline differentiator (§H). |
| REQ-DP-5 | Configuration and upgrade management. | Operability. |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-DP-1 | Trivial single-node bring-up (minutes, no broker required). | Open-core usability. |
| NFR-DP-2 | Elastic scaling of the Runtime worker pool under load. | Scale. |
| NFR-DP-3 | Safe rolling upgrades that do not drop in-flight runs. | Reliability. |
| NFR-DP-4 | Clean core/periphery split in packaging — no crippling of the core. | ASF requirement + ethics. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Deployment topology & operator scope
- **Context**: Single-node binary + basic chart (core) vs. operator + autoscaler (periphery); the autoscaling signal source. **ADR**: WEIR-A-0023.

### Decision Area: Work-distribution coupling / open-core packaging boundary
- **ADR**: WEIR-A-0010, WEIR-A-0005.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| WEIR-A-0023 | Deployment topology & operator scope | proposed | Core vs. periphery; autoscaling signal. |
| WEIR-A-0010 | Work distribution | proposed | Provides the scaling signal. |
| WEIR-A-0005 | Open-core / periphery boundary | proposed | Packaging boundary; no crippling. |
