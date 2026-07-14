---
id: 001-domain-config-data-model
level: adr
title: "Domain/config data model"
number: 1
short_code: "WEIR-A-0007"
created_at: 2026-06-17T02:11:56.374440+00:00
updated_at: 2026-06-22T20:38:53.528720+00:00
decision_date:
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0007: Domain/config data model

**Status:** Decided (2026-06-22). The connection-centric, two-tier weir-owned model is **realized + proven**:
Tier-1 (`Connection`/`StreamConfig` with `ConnectorRef`, config, schedule, sync mode, cursor, mapping) +
Tier-2 (`Run`/`WorkUnit`, `SyncState`+`Checkpoint`, `Outbox`, run history) all in the diesel-dualdb store,
with the **checkpoint + outbox + dead-letter single-transaction** boundary confirmed (the engine commits
them atomically, [[WEIR-A-0011]]). **Tenant-scoping** (schema-per-tenant) is the one open item and is
deferred to [[WEIR-A-0004]] (multi-tenancy) — it layers onto this model, not a blocker for deciding it.
*Raised by: [[WEIR-S-0002]] Control Plane, [[WEIR-S-0009]] Metadata & State Store.* *Decision-maker: Dylan Storey.*

## Context **[REQUIRED]**

The Control Plane and Store need one authoritative model of the core entities so configuration has a single owner. Because `cloacina` is **not** a dependency ([[WEIR-A-0002]] Dependency hygiene), there is **no external orchestrator that owns run state** — weir owns the *entire* model: desired-state config **and** runtime/operational state, all in the `diesel-dual-db` store ([[WEIR-A-0009]]).

## Decision **[REQUIRED]**

**Connection-centric model, with config and runtime state both owned by weir's store, in two tiers:**

### Tier 1 — Config (desired state)
```
Workspace ──┬── Source ──────┐
            ├── Destination ──┴── Connection ── StreamConfig[]
            └── Schedule                          (selection, sync mode, cursor field,
                                                   mapping → WEIR-A-0026)
Connection → ConnectorRef (pinned connector version, WEIR-A-0019)
```
- A **Connection** is the sync unit: source → destination. **Reverse-ETL is symmetric** — a destination may be a warehouse *or* an operational system; no separate entity.
- **StreamConfig** carries per-stream selection, sync mode, cursor field, and the in-flight mapping spec ([[WEIR-A-0026]]).
- All Tier-1 entities are **tenant-scoped** ([[WEIR-A-0004]]) with **optimistic concurrency** on edits (NFR-CP-4).

### Tier 2 — Runtime / operational state (also weir-owned)
- **Run** + **WorkUnit** — execution records produced by the Sync Engine ([[WEIR-S-0004]]).
- **Per-stream SyncState + Checkpoint** — opaque connector state + cursor, **committed transactionally** by the Engine ([[WEIR-A-0011]]).
- **Outbox** — the work-distribution intent log ([[WEIR-A-0010]]), written in the **same transaction** as the checkpoint commit (this atomicity is the whole point of the outbox).
- **RunHistory** — durable projection for the UI/observability ([[WEIR-S-0011]]).

Tiers 1 and 2 live in the same store so that "advance checkpoint + emit work + record run state" is one atomic transaction.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Connection-centric, weir owns all state (chosen) | Matches user mental model; symmetric reverse-ETL; one transactional store enables outbox atomicity | Mapping/selection complexity sits on the connection; weir owns run-state (no orchestrator to offload to) | **Chosen** |
| Pipeline/DAG-centric | Flexible composition | Over-general for a sync product; scope creep toward general orchestration (Out-of-Scope) | Rejected |
| Run state in an external orchestrator (e.g., depend on cloacina) | Less to build | Rejected by [[WEIR-A-0002]] — no general-orchestrator dependency | Rejected |

## Rationale **[REQUIRED]**

A connection (source→destination) is the natural unit and supports reverse-ETL symmetrically. Owning runtime state in the same store as config is what makes the outbox pattern's atomicity ([[WEIR-A-0010]]/[[WEIR-A-0011]]) possible — the checkpoint, the work emission, and the run record commit together. A DAG-centric model would pull us toward general orchestration we explicitly delegate to Airflow.

## Consequences **[REQUIRED]**

### Positive
- One owner, one store; transactional coherence across config + checkpoint + outbox + run state.
- Symmetric ingestion/activation; clean connection mental model.

### Negative
- weir owns run/execution state (no orchestrator to offload to) — more schema to design and maintain.
- Stream selection + mapping carry real complexity on the connection.

### Neutral
- Persisted via `diesel-dual-db` ([[WEIR-A-0009]]); tenant-scoped ([[WEIR-A-0004]]); mapping per [[WEIR-A-0026]]; connector pinning per [[WEIR-A-0019]].

## Review Schedule **[CONDITIONAL: Temporary Decision]**

### Exit criteria (resolved 2026-06-22):
- ✅ Concrete entity definitions + Diesel migrations (Tier 1 + Tier 2) running on both SQLite and Postgres —
  shipped (the diesel-dualdb store `migrate()`; connections, runs/work_units, stream_state, outbox,
  dead_letters, run_logs).
- ✅ Single-transaction boundary spanning checkpoint + outbox (+ dead-letters) — confirmed in the engine
  ([[WEIR-A-0011]]); proven by tests.
- ⏭️ Tenant-scoping (schema-per-tenant) — **deferred to [[WEIR-A-0004]]** (multi-tenancy). It layers onto
  this model at the access layer; the data-model decision does not block on it. Current MVP is single-tenant
  (no `Workspace` entity yet).
