---
id: signal-broker-delivery-plane
level: specification
title: "Signal Broker / delivery plane"
short_code: "WEIR-S-0017"
created_at: 2026-07-09T12:46:42.490914+00:00
updated_at: 2026-07-09T12:46:42.490914+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#specification"
  - "#phase/discovery"


exit_criteria_met: false
initiative_id: NULL
---

# Signal Broker / delivery plane

> **New component identified during Signal Fabric enablement.** The runtime behind the F2 pub/sub destination
> ([[WEIR-I-0036]]) is not an ordinary connector — it is a new host-side service. Execution model **decided** in
> [[WEIR-A-0038]] (2026-07-09): standalone host-side native process; **pure push-based socket subscription fan-out
> on async-runtime primitives — not a log/queue broker**. This spec is in **discovery**; it frames the component
> and its remaining open decisions.

## Overview **[REQUIRED]**

The **Signal Broker** is weir's **delivery plane for live named signals**: a **standalone, resident, host-side**
process (native Rust, alongside the engine/orchestrator — [[WEIR-A-0038]]) that receives records published by the
sync engine (the write side of the F2 pub/sub destination, [[WEIR-I-0036]]) and **pushes each out to all
currently-subscribed consumers over long-lived sockets**. It is the runtime behind the F2 destination and the
transport that F5's catalog ([[WEIR-I-0039]]) advertises.

**It is a pure push-based subscription broker to sockets — not a log/queue broker.** No retention, no offsets, no
replay; **"lag" is not a concept**. A subscriber that cannot keep up has the record dropped for its socket (it gets
the next push / current value) or is evicted if the socket is dead. The producer and other subscribers never wait —
slow-consumer isolation is **structural**.

**Open-core scope line.** The broker provides **generic named channels + socket fan-out**. What a signal name
*means* (the Swish naming/namespace scheme) and fabric subscription semantics are layered by the **Swish extension
in Swish repos** ([[WEIR-A-0005]]), **not** built into the broker.

## System Context **[CONDITIONAL: System-Level Spec]**

### Actors
- **Producer (sync engine / F2 destination facade):** lands a published record into a named channel; **unaware of
  subscriber count** ([[WEIR-I-0036]]).
- **Subscriber (consumer):** opens a long-lived connection, subscribes to a named channel, **receives the current
  state then a stream of deltas** (see the state+delta shape below).
- **Catalog ([[WEIR-I-0039]] / [[WEIR-S-0007]]):** advertises which channels exist, their contract (schema +
  freshness [[WEIR-I-0037]]), producer identity, and liveness — the discovery surface consumers use **before**
  connecting.

### External Systems
- **Resident sources ([[WEIR-I-0035]]):** upstream producers feeding the engine; the broker sits **downstream of
  the sync engine**.
- **Computed producers ([[WEIR-I-0040]]):** appear as ordinary channels once their output enters the engine; the
  F6 "push" pattern needs a symmetric **inbound ingest** path (see Architecture Framing).

### Boundaries
- **In scope:** the generic named-channel registry; subscriber socket lifecycle (subscribe/unsubscribe);
  **push** one published record → N connected sockets; **retain exactly one current state (snapshot) per channel**;
  present a **current-state path and a delta path** per channel; slow/dead-subscriber **drop/evict** handling;
  per-tenant channel isolation ([[WEIR-A-0036]]).
- **Out of scope:** signal **naming/namespace semantics** (Swish extension); **interpreting** freshness / deciding
  staleness (consumer / Policy-Engine, per [[WEIR-I-0037]]); **any computation** over the stream ([[WEIR-V-0001]]
  charter — the broker **carries, never computes**); **message retention / offsets / replay / consumer-lag
  tracking** (this is a live tap + latest-state, **not a log**).

## The state + delta shape **[REQUIRED]**

Because there is no replay, a subscriber connecting mid-stream cannot reconstruct state from history. So **every
channel presents two paths**:

- **current-state (snapshot)** — the latest value, retained as a single snapshot per channel (`watch`-like).
- **delta** — the ongoing push of changes.

**Connect semantics:** on subscribe, the broker delivers the **current state**, then streams **deltas**, with no gap
between the two (**connect → current state + delta → go**). This is what makes a delta-oriented stream consumable at
all under a no-log model. A producer/source whose stream is **delta-only must define its snapshot** so the broker
can serve current state at connect time. The state/delta representation interplays with the freshness triple
([[WEIR-I-0037]]) and is an open contract-shape decision (below).

## Requirements **[REQUIRED]**

### Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| REQ-S17.1 | Accept records published to a named channel from the sync engine (F2 write facade). | Write side of the pub/sub destination. |
| REQ-S17.2 | Maintain subscriptions over long-lived connections; support subscribe/unsubscribe at any time. | Live consumers join/leave continuously. |
| REQ-S17.3 | **Push** each published record to all connected subscribers of its channel. | Core fan-out (async-runtime primitive, [[WEIR-A-0038]]). |
| REQ-S17.4 | On a subscriber that can't keep up, **drop the record for that socket** (it gets the next push / current value) or **evict** a dead socket — never stall the producer or other subscribers. | Push model; slow-consumer isolation is structural — no backpressure/lag. |
| REQ-S17.5 | Keep the producer unaware of subscriber count. | F2 acceptance — fan-out is the broker's job. |
| REQ-S17.6 | Expose channel existence + liveness for the catalog to advertise. | F5 discovery; the broker is the source of truth for "is this channel live." |
| REQ-S17.7 | Retain **exactly one current state (snapshot) per channel** and serve it at connect. | State+delta shape; joinable mid-stream without a log. |
| REQ-S17.8 | Present a **current-state path and a delta path** per channel; on subscribe deliver current state then stream deltas with no gap. | State+delta shape (connect → state + delta → live). |
| REQ-S17.9 | Provide an inbound ingest path for computed producers that push (F6 push pattern). | F6 "service pushes to weir." |

### Non-Functional Requirements

| ID | Requirement | Rationale |
|----|-------------|-----------|
| NFR-S17.1 | Upstream read cost paid **once per source cycle**, independent of subscriber count. | F4 read-amplification collapse. |
| NFR-S17.2 | Per-subscriber delivery cost low and roughly linear; sustain the target fan-out ratio **[TARGET TBD — set with Swish E&I]**. | F4 scale. |
| NFR-S17.3 | The source cadence is **structurally decoupled** from subscribers — a slow subscriber is dropped, never throttled upstream (no backpressure path exists). | F4 isolation, made structural by the push/drop model. |
| NFR-S17.4 | Per-tenant channel isolation. | [[WEIR-A-0036]] tenant-isolated execution. |

## Architecture Framing **[CONDITIONAL: System-Level Spec]**

### Decision Area: Execution model — **RESOLVED**
Standalone host-side native process; push-based socket subscription fan-out on async-runtime primitives; not a
log/queue broker; not a wasm guest. → [[WEIR-A-0038]] (**decided** 2026-07-09).

### Decision Area: Fan-out engine (build vs adopt) — **RESOLVED**
Build on the async runtime's primitives (tokio `broadcast`/`watch`/`mpsc`); no embedded or external broker. →
[[WEIR-A-0038]].

### Decision Area: State/delta contract representation — **OPEN**
- **Context:** how "current state" is represented, how a delta relates to it, and how the snapshot→delta handoff is
  made gap-free; how a delta-only source declares its snapshot.
- **Constraints:** interplays with the freshness triple ([[WEIR-I-0037]]); must be uniform across connector types.
- **ADR:** open — likely a contract-shape ADR alongside F3.

### Decision Area: Subscriber protocol — **OPEN**
- **Context:** wire protocol + subscription API carrying both the state and delta paths; versioning.
- **Constraints:** must interop with the shared Swish client library that reads freshness ([[WEIR-I-0037]]).
- **ADR:** open.

### Decision Area: Inbound posture (shared with F6 push) — **OPEN**
- **Context:** subscriber sockets **and** F6 push-ingest are both new **inbound** surfaces; do they share one layer?
- **Constraints:** subscriber/producer auth; connection limits; tenant isolation ([[WEIR-A-0036]]); relation to the
  control-plane API ([[WEIR-S-0002]]).
- **ADR:** open.

## Decision Log **[CONDITIONAL: Has ADRs]**

| ADR | Title | Status | Summary |
|-----|-------|--------|---------|
| [[WEIR-A-0038]] | Delivery-plane execution model | **decided** | Standalone host-side native broker; push-based socket subscription fan-out on async primitives; not a log/queue; state+delta shape. |

## Constraints **[CONDITIONAL: Has Constraints]**

### Technical Constraints
- Host-side **native Rust**, standalone process; **not** a WASM connector guest ([[WEIR-A-0030]] / [[WEIR-A-0038]]).
- **Push-based socket fan-out, not a log/queue** — no retention beyond one current-state snapshot per channel; no
  offsets/replay; lag not modeled.
- weir's **first inbound network surface** — subscriber auth, connection limits, tenant isolation ([[WEIR-A-0036]]).
- **Carries** records; never computes over or decides on them ([[WEIR-V-0001]] charter, as amended).

### Organizational Constraints
- Generic named channels + fan-out are **open-core**; the Swish naming/subscription semantics are
  **extension-in-Swish-repos** ([[WEIR-A-0005]]).
