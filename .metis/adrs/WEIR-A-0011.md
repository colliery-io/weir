---
id: 001-delivery-checkpoint-semantics
level: adr
title: "Delivery & checkpoint semantics"
number: 1
short_code: "WEIR-A-0011"
created_at: 2026-06-17T02:12:04.884262+00:00
updated_at: 2026-06-21T22:36:37.309865+00:00
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

# ADR-0011: Delivery & checkpoint semantics

**Status:** Accepted (ratified 2026-06-21). *Raised by: [[WEIR-S-0004]] Sync Engine.*

**Realized by** [[WEIR-I-0004]] (Postgres Upsert/business-keys idempotency) + [[WEIR-I-0006]] /
[[WEIR-A-0029]] (the v1 streaming contract): the engine commits `stream_state` + outbox + dead-letters
in **one transaction on each `Checkpoint` message**; on error it returns and retries from the last
committed state (at-least-once); replay safety comes from idempotent destination writes
(`WriteMode::Upsert` on business keys). This ADR ratifies what is implemented.

## Context **[REQUIRED]**

Crash-safe syncs require clear delivery semantics: what happens on retry, and how checkpoints relate to durably-completed work. Exactly-once across arbitrary external systems is generally infeasible.

## Decision **[REQUIRED]**

*Decided:* **at-least-once dispatch with idempotent checkpointing** — no checkpoint advances past work that did not durably complete; replays are safe because writes are idempotent (upsert/dedup on business keys for reverse-ETL, per capabilities §C).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| At-least-once + idempotent (chosen) | Practical, crash-safe; matches reverse-ETL idempotency | Requires idempotent writes downstream | Medium | Medium |
| Exactly-once attempt | Strong guarantee | Infeasible across heterogeneous external systems | High | High |

## Rationale **[REQUIRED]**

At-least-once with idempotent checkpoints is the industry-proven, achievable contract and dovetails with first-class idempotent activation.

## Consequences **[REQUIRED]**

### Positive
- Crash-safe and tractable across diverse destinations.

### Negative
- Connectors/destinations must support idempotent writes.

### Neutral
- Interacts with partition planning ([[WEIR-A-0012]]).
