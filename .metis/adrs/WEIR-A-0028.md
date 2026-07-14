---
id: 001-async-orchestration-boundary-sync
level: adr
title: "Async orchestration boundary, sync execution core"
number: 1
short_code: "WEIR-A-0028"
created_at: 2026-06-18T16:43:54.876255+00:00
updated_at: 2026-06-18T16:44:46.733358+00:00
decision_date: 2026-06-18
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-1: Async orchestration boundary, sync execution core

**Status:** Decided (2026-06-18, Dylan Storey).

## Context **[REQUIRED]**

Building the run-orchestration layer ([[WEIR-S-0004]], realizing the transactional outbox [[WEIR-A-0010]] and out-of-process agents [[WEIR-A-0002]]) forces a runtime-concurrency decision. We are adopting cloacina's work-execution model largely verbatim ([[WEIR-A-0027]] pattern-not-dependency): a `Dispatcher` routes a lightweight ready-event to a `WorkExecutor`; executors claim work from the shared store with a heartbeat lease; the model spans an in-process executor and a future out-of-process **agent fleet**. cloacina's traits are `async` (tokio).

weir's per-work-unit work is **I/O-bound and blocking at the leaf**: a connector call is synchronous FFI into a `fidius` plugin (which does HTTP/DB internally), and the durable store is **diesel-dualdb (synchronous diesel)**. So the question is not "is the work async" (it isn't) but "where does the async boundary sit."

## Decision **[REQUIRED]**

**The orchestration layer is async (tokio); the execution core is sync. The boundary is explicit and one-directional.**

- **Async, in `weir-orchestrator` only:** the `Dispatcher`, `WorkExecutor` trait, the claim/heartbeat **relay**, the scheduler loop — everything that manages *many in-flight units* (timeouts, cancellation, heartbeats, backpressure/semaphores) and that a remote executor must speak over the network.
- **Sync, everywhere below:** the `Engine` (read→map→write→checkpoint) and the **connector contract** (`fidius` plugins). Connector authors never write `async fn` or see a runtime — blocking FFI is honestly blocking.
- **Bridge:** the `InProcessExecutor` runs the sync `Engine` via `tokio::task::spawn_blocking`. `diesel-dualdb` is used by wrapping each DB interaction as a discrete blocking unit — **never hold a pooled `DualConnection` across an `.await`** (acquire → work → drop, inside the blocking section), so the DB pool and blocking pool stay decoupled from the async scheduler.
- **Throughput model:** per-agent concurrency = an admission **semaphore** gating a bounded **`spawn_blocking` pool** (the real knob for blocking connector calls); macro throughput = the **agent fleet** (more processes/machines claiming from the shared DB queue). Async is the substrate that makes both clean and makes a `RemoteExecutor` natural — it is not itself the source of raw throughput.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk |
|--------|------|------|------|
| **Async orchestration, sync core (chosen)** | Clean timeouts/cancel/heartbeat/backpressure; natural remote-executor seam; ports cloacina directly; connector authoring stays sync | tokio in the orchestrator; `spawn_blocking` bridge; orchestrator tests are async | Low |
| Fully sync (threads + channels) | No tokio; simplest deps | Hand-rolled timeout/cancel/heartbeat; `block_on` at every remote boundary; diverges from cloacina | Medium |
| Fully async incl. connectors / diesel-async | Uniform async | **Colors the entire plugin ecosystem** for zero execution gain (work is blocking FFI); `diesel-async` doesn't compose with dualdb's `MultiConnection` today | High |

## Rationale **[REQUIRED]**

- Async's value here is the **control plane**, not the work: structured concurrency, per-unit timeout + cancellation, heartbeats, and semaphore backpressure are exactly what the orchestrator needs and are painful to hand-roll.
- It makes **out-of-process** (the explicit early goal, [[WEIR-A-0002]]) natural: network dispatch is async; a sync seam would force `block_on` at the boundary.
- Keeping the **connector contract sync** preserves a dead-simple authoring model and avoids async-coloring a community plugin surface for no benefit, since the leaf work is blocking FFI regardless.
- Adopting cloacina's async shape makes "largely bring it over" a port, not a translation.

## Consequences **[REQUIRED]**

### Positive
- One crate owns tokio (`weir-orchestrator`); the rest of the stack stays sync and simple.
- The `WorkExecutor` trait is the swap point: `InProcessExecutor` now, `RemoteExecutor` later, relay unchanged.
- Connector authors and the `Engine` are unaffected by the concurrency model.

### Negative
- `spawn_blocking` bridging friction; function-coloring at the boundary; orchestrator tests use `#[tokio::test]`.
- The `spawn_blocking` pool size (not the async runtime) is the per-agent concurrency ceiling and must be sized deliberately.

### Neutral
- DB-connection discipline (no connection held across `.await`) becomes a reviewable invariant.
- `diesel-async` remains a future option if a fully-async store is ever warranted; not needed for the outbox/claim cadence.
