---
id: f5-catalog-as-live-signal-registry
level: initiative
title: "F5 — Catalog as live-signal registry"
short_code: "WEIR-I-0039"
created_at: 2026-07-09T03:19:40.617875+00:00
updated_at: 2026-07-09T03:19:40.617875+00:00
parent: WEIR-V-0001
blocked_by:
  - WEIR-I-0035
  - WEIR-I-0037
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: f5-catalog-as-live-signal-registry
---

# F5 — Catalog as live-signal registry Initiative

> **Feature request — Signal Fabric enablement (F5 of 6).** **Home: open-core** (extends the existing connector
> catalog). Depends on [[WEIR-I-0035]] (F1, producers to register) and [[WEIR-I-0037]] (F3, the contract the catalog
> records). Extends [[WEIR-S-0007]] (Connector Catalog / Registry). Filed in **discovery**.

## Context

Extend weir's **existing** connector catalog so it can represent and expose **live named signals**, not only
batch/CDC connector definitions. The catalog becomes the **discovery surface**: given a signal name, what is its
contract (schema + freshness), who produces it, and is it currently live. A prospective consumer browses/queries the
catalog to find what it can bind to.

## Goals & Non-Goals

**Goals:**
- A resident source **registers its signal(s) in the catalog on start**.
- A consumer can **query the catalog by name** and retrieve the signal's contract (schema + freshness fields) and
  producer identity.
- The catalog **reflects liveness** — a dead producer's signal is still discoverable but **marked not-live**
  (last-seen).
- **Discovery is read-cheap** — resolving a name against the catalog incurs **no source-side cost**.

**Non-Goals:**
- **Registry ≠ transport.** The catalog records *what exists and its contract*; it is **not** the delivery path
  (that is [[WEIR-I-0036]], F2).
- Building a **parallel registry** — reuse the existing catalog; a live signal is a catalog entry with a
  resident-source producer and a freshness-carrying contract.
- Defining what signal **names mean** (namespace / canonical-identity scheme) — that is a Swish design decision;
  weir carries names, it does not define them (see "Explicitly NOT weir").

## Open-Core Boundary

**Wholly open-core.** A signal registry over the existing catalog is a general capability — anyone running live
sources benefits from name→contract→producer→liveness discovery. The **naming scheme itself** (how names key on
canonical identity, venue, counterparty) is a **Swish** concern and is not filed against weir.

## Detailed Design

*(Discovery-phase sketch.)*

- **Catalog entry shape.** Extend the catalog record ([[WEIR-S-0007]], and see storage backend [[WEIR-A-0018]]) so
  an entry can be a live signal: name, contract (schema + F3 freshness fields), producer identity, liveness/last-seen.
- **Registration on start.** A resident source (F1) registers its signal(s) when it comes up; deregistration /
  not-live marking is driven by the F1 supervisor's liveness signal + F3 heartbeat.
- **Cheap reads.** Query-by-name resolves against the catalog store only — never round-trips to the producing
  source (the read-cheap guarantee).

## Alternatives Considered

- **A separate live-signal registry alongside the catalog.** Rejected — the memo requires reuse of the existing
  catalog; a parallel registry duplicates discovery and drifts.
- **Deriving liveness by probing producers at query time.** Rejected — violates read-cheap; liveness comes from
  registration + heartbeat state, not query-time probing.

## Acceptance Criteria

- [ ] A resident source registers its signal(s) in the catalog on start.
- [ ] A consumer can query the catalog by name and retrieve the signal's contract and producer.
- [ ] The catalog reflects liveness (a dead producer's signal is discoverable but marked not-live).

## Open Questions

- How liveness/last-seen is kept current cheaply — pushed from the F1 supervisor / F3 heartbeat vs. a TTL the
  catalog ages out.
- Catalog storage implications ([[WEIR-A-0018]]) of holding mutable liveness state next to (largely static)
  connector definitions.
- The query/browse surface exposed to consumers (control-plane API [[WEIR-S-0002]] and/or UI [[WEIR-S-0003]]).

## Implementation Plan

Discovery deliverable — ratify the catalog-entry extension + liveness model, then decompose. Candidate seams:
(1) catalog schema extension for live signals; (2) register-on-start / mark-not-live wiring to the F1 supervisor +
F3 heartbeat; (3) read-cheap query-by-name API; (4) surfacing liveness in the API/UI.

**Exit criteria:** catalog extension + liveness model ratified with the human; F1 + F3 shapes resolved enough to
register against; decomposable into tasks.
