---
id: 001-delivery-plane-out-of-scope-weir
level: adr
title: "Delivery plane out of scope — weir carries and lands signals, it is not a subscribable fabric"
number: 40
short_code: "WEIR-A-0040"
created_at: 2026-08-04T14:41:03.811449+00:00
updated_at: 2026-08-04T14:41:03.811449+00:00
decision_date: 2026-08-04
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0040: Delivery plane out of scope — weir carries and lands signals, it is not a subscribable fabric

**Status:** Decided (2026-08-04, Dylan Storey). **Reverses the delivery-plane half of the Signal Fabric enablement
program.** Supersedes [[WEIR-A-0038]] (delivery-plane execution model); closes and archives the Signal Broker spec
[[WEIR-S-0017]] and the F2/F4/F5/F6 initiatives [[WEIR-I-0036]], [[WEIR-I-0038]], [[WEIR-I-0039]], [[WEIR-I-0040]];
re-tightens the Constraints amendment in the vision ([[WEIR-V-0001]]). *Raised by: a scope review — the "subscribable
binding layer" direction was judged too far from what weir is.*

## Context **[REQUIRED]**

The Signal Fabric enablement program (F1–F6) grew a second identity for weir. F1 ([[WEIR-I-0035]], **completed**)
gave weir **long-lived resident sources** — a source that stays up and emits, dialing **outbound** to poll or tail a
high-frequency upstream and land the data. That is squarely weir: *ingestion and reverse-ETL of data that flows in
and gets written down.*

The rest of the program pushed past that line. F2 ([[WEIR-I-0036]]) pub/sub destination, F4 ([[WEIR-I-0038]])
fan-out at scale, F5 ([[WEIR-I-0039]]) catalog-as-live-signal-registry, and F6 ([[WEIR-I-0040]]) consume-a-service-
output turned weir into something consumers **subscribe to and query** — a push-based **delivery plane** with named
channels, inbound subscriber sockets, and a snapshot+delta contract. [[WEIR-A-0038]] made that a first-class,
standalone, host-side **Signal Broker** ([[WEIR-S-0017]]) — weir's *first inbound network surface*, with its own
auth, connection limits, per-tenant channel isolation, and independent scaling profile.

Two facts frame the reversal:
1. **None of the delivery plane is built.** F2/F4/F5/F6 and S-0017 are all in **discovery** with zero tasks; the only
   *decided* artifact is A-0038. There is no delivery-plane, broker, subscriber, or fan-out code in the tree. This is
   a scope correction in the system of record, not a code revert.
2. **The delivery plane is a different product.** A subscribable, addressable, current-value push broker is a
   message-fabric — a category weir's charter deliberately pairs *with* (Kafka/Flink/NATS), not one it becomes. It
   drags in an inbound network posture, a broker component to operate, and a fabric-wide snapshot+delta contract
   requirement on every producer — a large surface serving an ambition (weir-as-signal-fabric) that is out of scope.

## Decision **[REQUIRED]**

**weir carries a live signal *in* and lands it; it does not serve signals *out* to subscribers.** The delivery
plane — pub/sub destination, socket fan-out, the Signal Broker, the live-signal registry, and the inbound
consume-a-service endpoint — is **out of scope**. Concretely:

- **Supersede [[WEIR-A-0038]].** The delivery-plane execution model (standalone host-side broker, inbound sockets,
  snapshot+delta channel contract) is withdrawn. weir has **no inbound signal-serving network surface**.
- **Close and archive** the delivery-plane program: [[WEIR-I-0036]] (F2), [[WEIR-I-0038]] (F4), [[WEIR-I-0039]]
  (F5), [[WEIR-I-0040]] (F6), and the Signal Broker spec [[WEIR-S-0017]].
- **Re-tighten the vision.** The 2026-07-08 Constraints amendment ([[WEIR-V-0001]]) that put *"socket fan-out to
  many subscribers"* in scope is reverted; fan-out-to-subscribers returns to **out of scope**. What stays in scope is
  only *"long-lived resident sources that stay up and emit"* (F1) — carry a signal in and land it.

**What is retained (the line holds at F1):**
- **F1 — resident sources ([[WEIR-I-0035]], completed).** Continuous-polling and event-reader sources that stay
  resident and emit, under supervision. This is the high-frequency data-gathering weir keeps.
- **A-0039 — brokered websocket egress ([[WEIR-A-0039]]).** A resident source dialing an **outbound** `ws`/`wss`
  upstream over host-brokered TCP. Outbound client, same posture as `wasi:http` egress — unaffected.
- **F3 — freshness triple ([[WEIR-I-0037]]), re-scoped.** Retained but **shorn of its delivery-plane framing**. The
  snapshot+delta / subscribe-mid-stream shape (which existed only to serve the broker) is dropped. What remains is a
  connector-contract amendment supporting **resident sources**: the freshness triple (value timestamp, liveness
  heartbeat, source-declared staleness) plus the `resident_capable` / `event_reader` `spec()` fields that were
  deferred out of F1 into this contract change. F3 stands as an F1-supporting contract nicety, not a fabric feature.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **A. Delivery plane out of scope; keep F1 + re-scoped F3 (DECIDED)** | Restores a single, sharp product identity (ingest + reverse-ETL + resident gathering); no inbound network surface to secure; no broker to operate; nothing to un-build | Forecloses the "the fabric IS weir" ambition; consumers wanting fan-out pair weir with a real broker | Low | S (docs only) |
| **B. Keep the plan, defer it** | Preserves optionality | Leaves a decided ADR + five discovery docs advertising a direction we won't take; the vision keeps promising a subscribable fabric; scope stays ambiguous | Medium | — |
| **C. Build a thin fan-out only** | Smaller than the full broker | Still weir's first inbound surface + the snapshot/delta fabric contract; the hard parts (auth, isolation, current-state) are inherent, not trimmable | High | L |

## Rationale **[REQUIRED]**

Option **A**. The delivery plane was the point where weir stopped being an ingestion/reverse-ETL platform and started
becoming a message fabric — a category with its own incumbents that the charter pairs with rather than replaces.
Because none of it is built, the cost of stopping is a documentation correction; the cost of *not* stopping is a
standing invitation (a decided ADR, five discovery initiatives, a live vision clause) to build weir's first inbound
network surface and a fabric-wide contract obligation. The clean boundary is exactly F1: **stay up, emit, and land** —
outbound-only, no subscribers. F3's freshness fields survive because they serve resident sources directly and carry
deferred F1 work; their broker-shaped framing does not.

## Consequences **[REQUIRED]**

### Positive
- One product identity again: weir ingests (including resident, high-frequency, outbound) and reverse-ETLs; it does
  not serve or query signals.
- No inbound network surface to design, secure, isolate per tenant, or operate; no standalone broker component.
- [[WEIR-A-0034]] ("destinations are wasm guests interpreting a manifest") describes **all** destinations again —
  the exception A-0038 carved out is withdrawn.
- The snapshot+delta obligation on every producer/channel disappears.

### Negative
- The "the fabric IS weir" thesis (Exchange ADR-007) is not realized in weir; a deployment wanting live fan-out to
  many subscribers pairs weir with a message broker (Kafka/NATS/Redis) downstream of a landed table or resident sink.
- Any external references to F2/F4/F5/F6 or the Signal Broker now point at archived/closed work.

### Neutral
- F1 ([[WEIR-I-0035]]) and A-0039 ([[WEIR-A-0039]]) are unaffected; resident sources and outbound ws egress remain.
- F3 ([[WEIR-I-0037]]) stays open but re-scoped; its snapshot/delta section is to be pruned when it leaves discovery.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

Permanent. Reopening the delivery plane would be a new ADR that supersedes this one.
