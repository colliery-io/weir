---
id: f6-consume-a-service-output-as-a
level: initiative
title: "F6 — Consume-a-service-output as a source (computed producers)"
short_code: "WEIR-I-0040"
created_at: 2026-07-09T03:19:40.666618+00:00
updated_at: 2026-07-09T03:19:40.666618+00:00
parent: WEIR-V-0001
blocked_by:
  - WEIR-I-0035
  - WEIR-I-0037
  - WEIR-I-0039
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: f6-consume-a-service-output-as-a
---

# F6 — Consume-a-service-output as a source (computed producers) Initiative

> **Feature request — Signal Fabric enablement (F6 of 6).** **Home: open-core** (a source type); the emitted
> **fabric contract** is the Swish extension's concern. Depends on [[WEIR-I-0035]] (F1, runtime), [[WEIR-I-0037]]
> (F3, contract) and [[WEIR-I-0039]] (F5, catalog registration). Filed in **discovery**.
>
> The **push** pattern ("service pushes to weir") needs an **inbound ingest** surface — a decision area shared with
> the Signal Broker ([[WEIR-S-0017]]) / [[WEIR-A-0038]]; the pull pattern is ordinary outbound and needs nothing new.

## Context

A source type that reads the output of an **external service** and emits it as a signal — so a **computed producer**
(a service that calculates a value: an aggregator, a scorer, a small transform/threshold service) publishes through
the same contract and appears on the fabric **indistinguishably** from a declaratively-polled source. **The service
computes; weir carries.** This is what lets the *"no middle"* rule hold: anything needing real computation is a
service whose output weir consumes, never computation pushed into weir.

Two patterns, both supported: the **service pushes** to weir, or **weir reads** a service endpoint. Either way weir
emits the result under the connector contract with freshness (F3).

## Goals & Non-Goals

**Goals:**
- A service's output can be wired in as a weir source with **minimal ceremony** — a clean *"here is my output + its
  freshness, carry it"* path, so a small single-purpose service is cheap to wire in (**not** a heavyweight
  connector-authoring exercise).
- The resulting signal carries the **freshness triple** (F3) and **registers in the catalog** (F5) like any other.
- A consumer binding to the signal **cannot tell** whether it is declaratively-polled or service-computed — same
  contract, same freshness, same catalog registration, same fan-out.

**Non-Goals:**
- **weir does not host the computation.** The service owns its own logic, deployment, and lifecycle; weir owns
  carrying the result. This boundary is what keeps weir from becoming a compute engine (per [[WEIR-V-0001]] charter).
- Defining the **fabric-specific shape** of what a computed producer emits (the Swish signal contract) — that is
  layered by the **Swish extension**, in Swish repos. weir provides the generic "consume a service's output as a
  source" mechanism only.

## Open-Core Boundary

**Split.** The **generic mechanism** — a source type that accepts/reads a service's output (push or pull) and emits
it under the connector contract with freshness — is **open-core**. The **fabric-specific contract shape** the Swish
computed producers emit is the **Swish extension's** concern, in Swish repos on weir's extension interface. weir
must not bake the Swish signal contract into this source type.

## Detailed Design

*(Discovery-phase sketch.)*

- **A lightweight source type** on the existing contract, running as a resident source (F1). Two ingress patterns:
  service→weir push (weir exposes an ingest endpoint) and weir→service pull (weir reads a declared endpoint on
  cadence). Both emit `ReadMessage`s ([[WEIR-A-0029]]) carrying F3 freshness.
- **Low ceremony.** The design bar is that a small service needs *"output + freshness"* and little else — contrast
  with full connector authoring ([[WEIR-S-0006]] / [[WEIR-S-0014]]). Discovering how light this can be while still
  reusing the contract is the core discovery question.
- **Indistinguishable downstream.** Emits through the same path so F5 registration and F2/F4 fan-out treat it
  identically to a declarative source.

## Alternatives Considered

- **Make computed producers author full connectors.** Rejected — the memo explicitly wants a cheap path so a
  single-purpose service isn't a heavyweight connector exercise.
- **Host the computation inside weir (a transform/compute stage).** Rejected — violates the charter boundary; weir
  carries, the service computes.

## Acceptance Criteria

- [ ] A service's output can be wired in as a weir source with minimal ceremony (not full connector authoring).
- [ ] The resulting signal carries the freshness triple and registers in the catalog like any other.
- [ ] A consumer binding to the signal cannot tell whether it is declaratively-polled or service-computed.

## Open Questions

- How light the "wire in a service output" path can be while still reusing the connector contract — config-only?
  a thin generic connector parameterized by endpoint/shape?
- Auth/secrets for the service endpoint (host-side credential injection, [[WEIR-A-0033]]/[[WEIR-A-0037]]).
- Exact extension seam where the Swish signal-contract shape is layered on.

## Implementation Plan

Discovery deliverable — ratify the generic mechanism + its core/extension seam, then decompose. Candidate seams:
(1) push-ingress endpoint source; (2) pull-endpoint source on cadence; (3) freshness (F3) + catalog (F5) wiring so
it's indistinguishable downstream; (4) the extension hook for the Swish signal contract.

**Exit criteria:** mechanism + core/extension boundary ratified with the human; F1/F3/F5 shapes resolved enough to
build against; decomposable into tasks.
