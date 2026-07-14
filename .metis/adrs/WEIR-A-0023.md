---
id: 001-deployment-topology-operator-scope
level: adr
title: "Deployment topology & operator scope"
number: 1
short_code: "WEIR-A-0023"
created_at: 2026-06-17T02:12:30.873542+00:00
updated_at: 2026-07-06T10:42:36.139006+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0023: Deployment topology & operator scope

**Status:** Decided (2026-07-06, dylan.storey) — finalized under [[WEIR-I-0021]]. *Raised by: [[WEIR-S-0012]]
Deployment & Operator.*

## Context **[REQUIRED]**

We must draw the core/periphery line in deployment: what ships open (single-node, basic K8s) vs. vendor (production operator, advanced autoscaler), and what signal drives autoscaling — without crippling the open core.

## Decision **[REQUIRED]**

**Core** = the single-node binary/compose **+** an open K8s topology: a standalone `weir runner` process, a
**per-tenant runner Deployment** ([[WEIR-I-0018]] isolation), a control-plane **k8s actuator** (`kube`-rs,
direct Deployments — **no CRD**) that provisions/scales those Deployments, and a **weir-owned autoscaler** —
leader-elected via a **store row** (portable), scaling each tenant's replicas on **queue depth**
(`Relay::pending_depth`, [[WEIR-A-0010]]) with scale-to-zero. Shipped as `charts/weir-server` +
`charts/weir-runner`. The k8s deps live behind a **`kubernetes` cargo feature** (default build stays light).

**Periphery** = a production Helm operator (lifecycle/upgrades), advanced/managed autoscaling (KEDA/HPA — a
`values.yaml` switch defers to them), and multi-cluster. Per [[WEIR-A-0005]].

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Basic autoscaling open, advanced periphery (chosen) | Core genuinely usable; clear vendor moat | Must define "basic vs advanced" precisely | Medium | Medium |
| All autoscaling periphery | Bigger vendor moat | Cripples the open core (violates open-core-stands-alone) | High | Low |
| All operations open | Maximal core | Erodes the vendor model | Low | High |

## Rationale **[REQUIRED]**

Basic autoscaling in the open core is a headline open-core capability; the production operator and advanced scaling are legitimate operational periphery per [[WEIR-A-0005]].

## Consequences **[REQUIRED]**

### Positive
- "Open core stands alone"; clear, ethical vendor moat.

### Negative
- The basic/advanced boundary needs precise definition.

### Neutral
- Signal source couples to work distribution ([[WEIR-A-0010]]); packaging per [[WEIR-A-0005]].
