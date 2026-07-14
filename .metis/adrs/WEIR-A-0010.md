---
id: 001-work-distribution
level: adr
title: "Work distribution"
number: 1
short_code: "WEIR-A-0010"
created_at: 2026-06-17T02:12:01.968909+00:00
updated_at: 2026-06-17T03:13:16.475267+00:00
decision_date: 2026-06-16
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0010: Work distribution

**Status:** Decided. *Raised by: [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0005]] Connector Runtime, [[WEIR-S-0012]] Deployment & Operator.* *Decision-maker: Dylan Storey, 2026-06-16.*

## Context **[REQUIRED]**

The Engine dispatches work units to the agent fleet ([[WEIR-A-0002]]) under concurrency limits; backlog depth is the autoscaling signal ([[WEIR-A-0023]]). The mechanism must not force a message broker on small deployments, must be atomic with state/checkpoint commits ([[WEIR-A-0011]]), and — per discovery — must **not** be a DB-as-concurrent-queue. The earlier draft of this ADR (Postgres `SELECT … FOR UPDATE SKIP LOCKED`) is **rejected**: the queue is not implemented as a SKIP-LOCKED relational queue.

## Decision **[REQUIRED]**

**Implement the transactional outbox pattern for work distribution in weir's own Sync Engine — adopting the pattern proven in `cloacina`, but NOT depending on `cloacina`** (it is a general orchestrator we do not embed; see Dependency note):

1. **Enqueue = transactional outbox write.** When the Engine plans/advances work, the work item is written to the **outbox in the same transaction** as the state/checkpoint commit. This is atomic — no dual-write, no lost or phantom work — and gives at-least-once delivery ([[WEIR-A-0011]]).
2. **Dispatch = relay → agent fleet.** A relay drains the outbox and distributes work units to weir's agent fleet ([[WEIR-A-0002]]). A single in-process relay+agent on single-node; relay feeding many agents when distributed.
3. **Not a DB polling queue, not a mandatory broker.** The outbox is a *write-once, relayed intent log*, not a contended `SKIP LOCKED` dequeue, and no external broker is required. weir persists the outbox via `diesel-dual-db` ([[WEIR-A-0009]]) — the durable record rides the existing store, but the *queue/dispatch* is the outbox+relay mechanism, not the relational DB acting as a queue.

**Dependency note.** weir implements this itself; `cloacina` ("Airflow, but better" — a general orchestrator) is a **pattern reference, not a dependency**. `diesel-dual-db` *is* a dependency ([[WEIR-A-0009]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Transactional outbox + relay, implemented in weir (cloacina pattern) (chosen) | Atomic with state (no dual-write); broker-free; pattern proven in cloacina; scales single→fleet; no heavy orchestrator dep | Relay is a component to operate; outbox needs draining/retention | **Chosen** |
| DB-as-queue (`SKIP LOCKED`) | No extra infra | Contended polling queue; non-portable across the dual-db; the rejected earlier draft | Rejected |
| Message broker (Kafka/NATS/Rabbit) | High throughput | Mandatory infra; breaks "no broker at small scale" (NFR-SE-5) | Rejected as default |

## Rationale **[REQUIRED]**

The transactional outbox gives the one property that matters most — **atomicity between state changes and work emission** — without a broker and without turning the relational store into a contended queue. The pattern is proven in `cloacina`; weir reimplements it on `diesel-dual-db` rather than taking `cloacina` as a dependency, keeping the open core free of a general-orchestrator dependency (which would also collide with the Out-of-Scope boundary on general workflow orchestration).

## Consequences **[REQUIRED]**

### Positive
- Atomic enqueue with checkpoints; at-least-once with no dual-write; broker-free; single-node stays one in-process process.
- Reuses the proven *pattern* (not cloacina as a dependency); keeps the core free of a general-orchestrator dep.

### Negative
- The relay is a component with its own liveness/retention concerns (outbox draining, dead-item handling).

### Neutral
- **Autoscaling signal** ([[WEIR-A-0023]]) becomes **outbox/backlog depth** rather than a queue length.
- Resolves the open item left in [[WEIR-A-0009]] ("must choose a non-SQLite/non-Postgres queue"): the queue/dispatch is cloacina's outbox+relay; the store only persists the outbox record.
