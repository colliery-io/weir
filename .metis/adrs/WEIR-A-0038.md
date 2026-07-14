---
id: 001-delivery-plane-execution-model
level: adr
title: "Delivery-plane execution model (resident, inbound, host-side)"
number: 38
short_code: "WEIR-A-0038"
created_at: 2026-07-09T12:46:34.939915+00:00
updated_at: 2026-07-09T12:46:34.939915+00:00
decision_date: 2026-07-09
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0038: Delivery-plane execution model (resident, inbound, host-side)

**Status:** Decided (2026-07-09, Dylan Storey). Arises from Signal Fabric enablement **F2** ([[WEIR-I-0036]]) /
**F4** ([[WEIR-I-0038]]); specified in [[WEIR-S-0017]]. Touches [[WEIR-A-0030]] (WASM-always) and [[WEIR-A-0034]]
(reverse-ETL destinations). *Raised by: the F2 pub/sub destination.*

## Context **[REQUIRED]**

The pub/sub destination (F2) publishes one source read to **many live subscribers over long-lived sockets**. The
feature memo frames it as *"the existing pluggable-destination interface."* That holds at the **authoring/config
seam** — a sync targets it like any destination — but **not at runtime**. weir's destinations today ([[WEIR-A-0034]])
are **WASM guests** ([[WEIR-A-0030]]: *only connectors are wasm; engine/orchestrator stay native Rust*) that run
**per sync-run** and make **outbound** `wasi:http` writes with host-brokered egress (the `EgressPolicy` seam,
[[WEIR-I-0034]]). A pub/sub broker inverts that: it must **hold inbound** subscriber sockets, be **resident** across
runs, and be an **addressable** endpoint consumers discover (F5, [[WEIR-I-0039]]) and connect to. So we must decide
what executes it — it **cannot** be an ordinary WASM destination guest.

The memo's *"backpressure / slow-consumer"* language also implied a **queue/log broker**. It is not one — see the
Decision.

## Decision **[REQUIRED]**

The delivery plane is a **new, first-class, standalone, host-side component — the Signal Broker ([[WEIR-S-0017]]) —
in native Rust, deployed as its own process.** It is **not** a WASM connector guest, and it is **not** folded into
the control plane: its fan-out data-path load and socket count scale independently of the API/scheduler.

**It is a pure push-based subscription broker to sockets — NOT a log/queue broker.** It holds live subscriber
sockets per named channel and pushes each published record straight to connected subscribers. There is **no
retention, no offsets, no cursors, no replay, and "lag" is not modeled.** A subscriber that cannot keep up has the
record **dropped for its socket** (it gets the next push / current value) or, if the socket is dead, is **evicted**.
The producer and other subscribers never wait — so **isolation from slow consumers is structural, not a tuning
parameter**: nothing buffers on a subscriber's behalf, so the source physically cannot be stalled by one.

**Fan-out uses the async runtime's own primitives** (tokio `broadcast` / `watch` / `mpsc`), **not** an embedded or
external message broker. The 1→N delivery weir needs is a runtime primitive; a broker would drag in the log/queue
machinery (retention, offsets, replay) this model explicitly rejects.

The **pluggable-destination interface is preserved**: the pub/sub destination a sync targets is a **thin facade**
whose `write` lands records into a named channel on the broker; the broker owns the subscriber-facing lifecycle.

**State + delta shape (fabric-wide consequence).** Because there is no replay, a subscriber connecting mid-stream
cannot reconstruct state from history. Therefore **every channel presents two paths: a current-state (snapshot) path
and a delta path.** Subscribe semantics are **snapshot-then-live with no gap**: on connect, receive the current
state, then receive deltas. A delta-oriented stream is thus always joinable mid-flight (**connect → current state +
delta → go**). The broker retains **exactly one current state per channel** (the latest snapshot — a single value,
à la `watch`, *not* a history), which is consistent with "not a log broker." This shapes **what anything on weir's
fabric presents**; the state/delta contract representation is detailed in [[WEIR-S-0017]] and interplays with the
freshness contract (F3, [[WEIR-I-0037]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **A. Standalone host-side native broker, async-runtime fan-out (DECIDED)** | Only model that holds inbound/resident sockets + cross-run subscriptions; keeps the authoring seam; native Rust fits the engine tier; scales independently; realizes "the fabric IS weir" | weir's first inbound network posture (auth, limits, tenant isolation) + a new component to deploy | Medium | L |
| **B. Pub/sub as an ordinary WASM destination guest** | No new component | Sandbox can't hold inbound sockets; per-run lifecycle can't hold cross-run subscriptions — breaks on every axis | High | Infeasible |
| **C. Embed/adopt a message broker (log/queue: NATS/Redis/Kafka/Iggy)** | Reuse proven machinery | This is a **pure push socket-subscription fabric, not a log/queue** — retention/offsets/replay are unwanted; and no viable *embeddable* Apache-2.0 Rust broker exists (they are standalone servers ⇒ Option D). The 1→N fan-out is an async-runtime primitive. | Medium | Wrong shape |
| **D. External broker weir doesn't own (operator wires Kafka/NATS)** | No broker code in weir | Undercuts "the fabric IS weir" (Exchange ADR-007); a hard external dep in the core signal path violates "the open core stands alone" ([[WEIR-A-0005]]) | High | — |

## Rationale **[REQUIRED]**

Option **A**. B is infeasible under the sandbox/lifecycle model; D breaks the open-core-stands-alone principle and
the fabric thesis. C is the wrong *shape*: a signal is a **current-value** thing (F3 freshness), so a delayed,
queued value is worthless — **push-latest-and-drop is the correct semantic and a durable log is actively wrong
here.** The fan-out to sockets is an async-runtime primitive, not a reason to take a broker dependency. Keeping it a
standalone native process lets the fan-out data path scale on its own profile (100s–1000s of long-lived sockets,
F4) without coupling to the control plane.

## Consequences **[REQUIRED]**

### Positive
- A clean home for named channels + socket fan-out; F4 scale is a property of one owned component.
- **Slow-consumer isolation is structural** (F4): the source cannot be stalled because nothing buffers per subscriber.
- The **current-state + delta** shape makes any channel joinable mid-stream **without a replay log**.
- Preserves one product mental model on the authoring side — a sync still "targets a destination."

### Negative
- weir's **first inbound network surface**: subscriber auth, connection limits, and **per-tenant channel
  isolation** ([[WEIR-A-0036]]) become real; a new standalone component to deploy/scale ([[WEIR-A-0023]]).
- [[WEIR-A-0034]]'s "destinations are wasm guests interpreting a manifest" no longer describes **all** destinations.
- **Fabric-wide shape requirement:** every producer/channel must present a current state (snapshot) **and** deltas;
  a source/contract that is **delta-only must define its snapshot**. New design surface ([[WEIR-S-0017]], interplays
  with F3 [[WEIR-I-0037]]).

### Neutral
- The broker retains **exactly one current state per channel** (a `watch`-like latest value), **not** history.
- **F6** ([[WEIR-I-0040]]) "service pushes to weir" needs a symmetric **inbound ingest** endpoint; whether it shares
  the broker's inbound layer or the control-plane API ([[WEIR-S-0002]]) is a spec question.

## Review Schedule **[CONDITIONAL: Temporary Decision]**

Permanent. **Prerequisite satisfied for F2 ([[WEIR-I-0036]]) to leave discovery.**
