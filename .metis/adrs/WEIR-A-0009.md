---
id: 001-durable-state-store
level: adr
title: "Durable state store"
number: 1
short_code: "WEIR-A-0009"
created_at: 2026-06-17T02:12:00.024660+00:00
updated_at: 2026-06-17T03:05:56.612944+00:00
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

# ADR-0009: Durable state store

**Status:** Decided. *Raised by: [[WEIR-S-0004]] Sync Engine, [[WEIR-S-0009]] Metadata & State Store.* *Decision-maker: Dylan Storey, 2026-06-16.*

## Context **[REQUIRED]**

The stateless services need one transactional source of truth for **relational metadata**: config domains, sync state + per-stream checkpoints, run history, and connector-catalog metadata. Deployment must span "single-node, no broker" up to multi-node scale without crippling the open core.

Two clarifications from discovery shape the decision:
- **The work queue is *not* part of this store.** Work distribution ([[WEIR-A-0010]]) is a separate mechanism and is explicitly **not** backed by SQLite or Postgres. This removes the `SELECT … FOR UPDATE SKIP LOCKED` dialect problem from the dual-backend store — the store only carries relational metadata, which ports cleanly.
- **Prior art exists.** `diesel-dual-db` is an existing, cloacina-proven crate that abstracts a Diesel access layer over both SQLite and Postgres. This is reuse, not new work.

## Decision **[REQUIRED]**

**One Diesel-based access layer over two backends via `diesel-dual-db` (existing, cloacina-proven): SQLite for the embedded/single-node local store now; PostgreSQL for the multi-node long-term target.**

- Scope: config (workspaces, sources, destinations, connections, schedules), sync state + per-stream checkpoints, run history, catalog metadata — all behind one typed interface, tenant-scoped ([[WEIR-A-0004]]).
- Combined **checkpoint + run-state updates are transactional** (satisfies NFR-SE-1 / NFR-ST-1) on both backends.
- Cross-backend portability is straightforward in Diesel for this workload (CRUD + `on_conflict` upserts work on both); **no work-queue dialect concern**, because the queue lives outside this store ([[WEIR-A-0010]]).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Diesel `diesel-dual-db` (SQLite + Postgres) (chosen) | Trivial single-node; scales to Postgres; **existing cloacina-proven crate**; no broker | Two backends to test | **Chosen** |
| Postgres-only | One backend | Single-node bring-up no longer trivial (violates NFR-DP-1) | Rejected |
| Custom store | Tailored | Reinventing durability; high risk | Rejected |

## Rationale **[REQUIRED]**

"The open core stands alone" demands a minutes-to-bring-up single-node path (SQLite), while real deployments need Postgres. `diesel-dual-db` already provides exactly this split and is proven in cloacina, so weir reuses it rather than rebuilding it. Pulling the work queue out of the relational store keeps the dual-backend layer free of the one genuinely non-portable construct (SKIP LOCKED).

## Consequences **[REQUIRED]**

### Positive
- Trivial single-node (SQLite); clean scale path to Postgres; no mandatory broker; reuses proven prior art.
- Dual-backend layer stays simple — only portable relational ops.

### Negative
- Dual-backend testing burden remains (must run the suite against both).

### Neutral
- Work distribution ([[WEIR-A-0010]]) is a **separate** mechanism, not this store — that ADR must now choose a non-SQLite/non-Postgres queue.
- Tenancy ([[WEIR-A-0004]]) is enforced through this access layer.
