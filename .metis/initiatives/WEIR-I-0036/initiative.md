---
id: f2-pub-sub-destination-fan-out-to
level: initiative
title: "F2 — Pub/sub destination (fan-out to many subscribers)"
short_code: "WEIR-I-0036"
created_at: 2026-07-09T03:19:40.475625+00:00
updated_at: 2026-07-09T03:19:40.475625+00:00
parent: WEIR-V-0001
blocked_by:
  - WEIR-I-0035
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: L
initiative_id: f2-pub-sub-destination-fan-out-to
---

# F2 — Pub/sub destination (fan-out to many subscribers) Initiative

> **Feature request — Signal Fabric enablement (F2 of 6).** **Home: open-core for the generic pub/sub destination;
> extension for the Swish-fabric-specific naming/subscription semantics.** Depends on [[WEIR-I-0035]] (F1) — a
> fan-out destination is only useful fed by a resident source. Filed in **discovery**.
>
> **New component.** The runtime behind this destination is **not** a WASM connector guest — it is a new host-side
> broker. See component spec [[WEIR-S-0017]] (Signal Broker / delivery plane) and execution-model ADR
> [[WEIR-A-0038]] (**decided 2026-07-09**): a standalone host-side native process; **pure push-based socket
> subscription fan-out on async-runtime primitives — not a log/queue** (no retention/offsets/replay; lag not
> modeled; a slow subscriber is dropped/evicted, never backpressured). Every channel presents a **current-state +
> delta** shape (connect → current state + deltas → go).

## Context

A destination type whose write target is **not a store but a set of live subscribers**. One source read is
published once; the destination fans it out to every current subscriber over a long-lived connection (socket).
This is the **symmetric mirror of weir's existing reverse-ETL destinations** — same pluggable-destination interface
([[WEIR-A-0034]]: reverse-ETL destinations on a shared declarative runtime), but the "write" is a
publish-to-subscribers.

The destination accepts a record from the sync engine and delivers it to all active subscriptions; it manages
subscription lifecycle (subscribe, unsubscribe, slow/dead-subscriber handling) so **the source side never sees
consumer count**.

## Goals & Non-Goals

**Goals:**
- A sync can target the pub/sub destination and deliver records to **multiple concurrent subscribers**.
- **Slow-consumer isolation:** one slow or dead subscriber does not stall the source or other subscribers.
- **Churn-safe:** subscriber join/leave does not interrupt delivery to others.
- The **source side is unaware of subscriber count** — fan-out is fully the destination's job.

**Non-Goals:**
- Stream *processing* on the delivery path (per [[WEIR-V-0001]] charter; carry, don't compute).
- The **scale target** (subscribers-per-source ratio, per-subscriber overhead) — that is [[WEIR-I-0038]] (F4),
  which hardens this destination + the F1 runtime to a measured ratio.
- Fabric-specific **signal naming / subscription semantics** — see Open-Core Boundary; these live in Swish repos.

## Open-Core Boundary

**Split.** The **generic pub/sub destination** (publish a record to named endpoints; manage sockets and
subscriptions; push-based fan-out with slow-consumer drop/evict — [[WEIR-A-0038]]) is **open-core** on the existing pluggable-destination
interface. What a signal *means* — the signal-name scheme, the freshness contract binding, subscription semantics —
is **layered by the Swish extension, in Swish repos, on weir's extension interface**, and is explicitly **not**
built into the core destination. Keeping this line clean is what protects the vendor-neutrality story
([[WEIR-A-0005]]).

## Detailed Design

*(Discovery-phase sketch.)*

- **On the destination interface, not a bypass.** Built on the same pluggable-destination seam as reverse-ETL
  ([[WEIR-A-0034]]); records arrive from the sync engine as they do for any destination — the sync engine is not
  bypassed.
- **Delivery over sockets;** the destination owns connection management: accept subscriptions, track active set,
  deliver each published record to all, evict/handle slow and dead subscribers.
- **Backpressure model.** Fan-out cost (1 source read → N deliveries) is the destination's concern; slow-subscriber
  backpressure must **not** couple back into source read cadence (the isolation guarantee F4 later measures).

## Alternatives Considered

- **A transport that bypasses the sync engine.** Rejected — the memo requires this ride the existing
  pluggable-destination interface.
- **Baking signal semantics into the core destination.** Rejected — violates the open-core boundary; fabric meaning
  is the extension's job.

## Acceptance Criteria

- [ ] A sync can target the pub/sub destination and deliver records to multiple concurrent subscribers.
- [ ] One slow or dead subscriber does not stall the source or other subscribers.
- [ ] Subscriber churn (join/leave) does not interrupt delivery to others.
- [ ] The source side is unaware of subscriber count.

## Open Questions

- Socket/protocol choice for the subscriber connection and its versioning story.
- Slow-consumer policy: buffer-then-drop, disconnect, or per-subscriber watermark — and where the boundary sits vs.
  F4's cadence-isolation guarantee.
- Exact shape of the extension seam the Swish naming/subscription layer plugs into.

## Implementation Plan

Discovery deliverable — ratify the design (esp. the core/extension seam) then decompose. Candidate seams:
(1) pub/sub destination on the pluggable-destination interface; (2) subscription lifecycle + socket management;
(3) slow/dead-subscriber isolation; (4) the extension hook the Swish signal layer binds to.

**Exit criteria:** design + core/extension boundary ratified with the human; F1 ([[WEIR-I-0035]]) sufficiently
resolved to feed it; decomposable into tasks.
