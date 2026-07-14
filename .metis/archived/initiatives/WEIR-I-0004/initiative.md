---
id: phase-2-full-connector-contract
level: initiative
title: "Phase 2: Full Connector Contract"
short_code: "WEIR-I-0004"
created_at: 2026-06-19T17:09:37.851185+00:00
updated_at: 2026-06-20T15:18:42.493521+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: phase-2-full-connector-contract
---

# Phase 2: Full Connector Contract Initiative

> **DISCOVERY DRAFT — for human review.** Scope, slices, and open questions below
> are proposed, not ratified. Do not decompose until signed off.

## Context **[REQUIRED]**

The vision ([[WEIR-V-0001]]) sequences **Phase 2 — Friction Fixes & Reverse ETL** after the Migratable Core. Its first thrust, chosen to lead per *interfaces-before-implementation* (Principle 3), is the **full connector contract** — the interface depth that first-class reverse ETL, parallel reads, and richer sources all build on. Builder + OpenLineage + the real reverse-ETL connectors (HubSpot/Salesforce) are deliberately **later** initiatives that consume this one.

**Key finding (grounds the scope):** the contract *types* ([[WEIR-S-0006]] / [[WEIR-A-0014]]) already declare the full surface —
`SyncMode::{FullRefresh, Incremental, Cdc}`, `WriteMode::{Append, Upsert{business_keys}, Overwrite}`, `PartitionScheme::{Unpartitioned, CursorRange, …}` — but the **engine honors none of them**: it does `FullRefresh` + `Append` + a single partition only. So this initiative is largely **implementing the already-decided interface** (engine + reference connectors + conformance tests), not redesigning it. The contract interface itself is unlikely to change → **probably no new ADR**, just spec updates marking each dimension implemented.

## Goals & Non-Goals **[REQUIRED]**

**Goals (proposed):**
- Engine **honors `WriteMode`**: `Append` (done), `Upsert{business_keys}` (dedup/merge by key), `Overwrite` (replace) — the reverse-ETL write prerequisite, proven on a reference destination.
- Engine **honors `SyncMode`**: explicit `Incremental` (cursor resume vs. full re-read) distinct from `FullRefresh`.
- Engine **honors partitioned reads**: a source declares partitions (`PartitionScheme::CursorRange`/keyset); the engine plans one work unit per partition over the orchestrator relay → **parallel reads** on the agent model.
- **Conformance tests** per dimension over a reference connector; the contract dimensions are documented as implemented in [[WEIR-S-0004]]/[[WEIR-S-0006]].

**Non-Goals (proposed):**
- Real reverse-ETL SaaS connectors (HubSpot/Salesforce) — *next* initiative; this proves the write *primitive* against Postgres.
- Connector builder UI + OpenLineage — separate Phase-2 initiatives.
- WASM `wasi:http` (blocked, [[WEIR-T-0023]]).
- *(`SyncMode::Cdc` is now IN scope as slice 4 — locked 2026-06-19; see Q1.)*

**New artifact:** a **`weir-connector-postgres`** (source + destination) is introduced as the real conformance vehicle — which also advances the connector-breadth goal. Validated against a docker-composed Postgres.

## Detailed Design **[REQUIRED — DRAFT]**

The contract types exist; the work is semantics in `weir-engine` + the reference connectors (`weir-connector-arrow-sink` for writes; a partitionable source) + the orchestrator for parallel partitions.

- **Write modes.** `Engine` passes `write_mode` to the destination via `WriteContext`; the reference sink implements `Upsert` (dedup/merge by `business_keys`) + `Overwrite` (truncate-then-write). At-least-once + upsert ⇒ idempotent merge (re-delivery safe). Decision: dedup in the engine vs. the connector — *proposed: the connector owns it* (destinations know their merge semantics), engine just conveys mode + keys.
- **Incremental.** Engine branches on `sync_mode`: `Incremental` resumes from the committed cursor (today's behavior generalized); `FullRefresh` ignores/clears the cursor. Reference incremental source asserts resume-from-cursor.
- **Partitioned reads.** A source advertises partitions (via `discover`/a plan step); the engine/relay plans **N work units** (one per partition), executed in parallel by the worker(s) — leaning on the agent-fleet model + the lease/heartbeat ([[WEIR-T-0018]]). Each partition checkpoints independently. This is the largest, riskiest slice.
- **CDC (slice 4).** A Postgres source variant reading via **logical replication** (a replication slot + `pgoutput`/`wal2json`). The WAL **LSN** is the resume point, carried in `StreamState.opaque` (bytes) rather than the text `cursor`. Honors `SyncMode::Cdc`. The harness Postgres must run with `wal_level=logical` (a docker-compose `command:` override). Engine already commits `opaque` state per chunk — so, like S1/S2, the connector does the honoring.

## Decisions / Open Questions **[updated 2026-06-19 from human input]**

- **Integration resources (decided).** Use **angreal tasks + docker-compose** to stand up real systems for conformance testing — primarily **Postgres**. A real **Postgres source + destination** connector becomes the vehicle that exercises every contract dimension against an actual database (not just a reference sink). This is a foundational slice (slice 0).
- **Q2 — partition ownership (resolved by the Airbyte model).** Airbyte has the **source declare partitions** via *partition routers / stream slicers* (`ListPartitionRouter`, `SubstreamPartitionRouter`, datetime→time-slices), with **per-partition cursor state** (global fallback past ~10k), slices doubling as checkpoint granularity. For migration fidelity, weir mirrors this: the **source advertises partitions** (a `stream_slices`/`plan_partitions`-equivalent contract call) and the engine checkpoints **per-partition**.
- **Q3 — ADR? → yes (small).** Partition planning + per-partition state is a *new contract capability* → a focused ADR (likely amends/extends [[WEIR-A-0014]] / [[WEIR-S-0006]]).
- **Q1 — CDC is IN scope (LOCKED 2026-06-19).** Slice 4 = Postgres **logical replication** CDC, with the WAL position (LSN) carried as the contract's **opaque** state (`StreamState.opaque`). Requires the harness Postgres at `wal_level=logical`. The harness makes it testable, so it stays in this initiative rather than deferring.
- **Q4 — slice order (LOCKED):** 0) harness → 1) write modes → 2) incremental → 3) partitioned/parallel reads (carries the ADR) → 4) CDC. S0–S2 done.

**Airbyte refs:** [partition router](https://docs.airbyte.com/platform/connector-development/config-based/understanding-the-yaml-file/partition-router) · [partitioning (builder)](https://docs.airbyte.com/platform/connector-development/connector-builder-ui/partitioning) · [incremental syncs](https://docs.airbyte.com/platform/connector-development/config-based/understanding-the-yaml-file/incremental-syncs) · [stream slices (CDK)](https://docs.airbyte.com/connector-development/cdk-python/stream-slices/)

## Alternatives Considered **[REQUIRED — DRAFT]**

- **Build reverse-ETL connectors first (skip contract depth).** Rejected per Principle 3 + the chosen direction — real connectors on an unfinished contract would bake in rework.
- **Redesign the contract.** Rejected — v0 types already cover these dimensions; this is honoring them, not redesigning.

## Implementation Plan **[REQUIRED — DRAFT, pending sign-off]**

Proposed slices (decompose after design sign-off):
0. **Integration harness** — `docker-compose` (Postgres) + angreal tasks (`integration up/down`) + a **`weir-connector-postgres`** source+dest as the real conformance vehicle. CI runs it via a Postgres service container; local devs via angreal.
1. **Write modes** — engine conveys `WriteMode`; Postgres dest does `Upsert` (`ON CONFLICT`) + `Overwrite`; idempotent-under-re-delivery conformance test.
2. **Incremental sync** — engine honors `SyncMode::Incremental` (resume-from-cursor) vs `FullRefresh`; Postgres source over a real cursor column.
3. **Partitioned/parallel reads** — source advertises partitions (Airbyte-aligned `stream_slices` equivalent) → N work units over the relay; **per-partition checkpoints**; parallel-execution test. *(carries the small ADR.)*
4. **CDC** *(LOCKED in scope)* — Postgres logical replication; WAL LSN as opaque resume state; harness Postgres at `wal_level=logical`.

## Exit Criteria

- [x] `WriteMode::{Append, Upsert, Overwrite}` honored, proven on the Postgres destination (idempotent upsert under re-delivery). — S1 / [[WEIR-T-0025]]
- [x] `SyncMode::{FullRefresh, Incremental}` honored (resume-from-cursor) with a conformance test. — S2 / [[WEIR-T-0026]]
- [x] A partitionable source fans out to parallel work units with independent per-partition checkpoints. — S3 / [[WEIR-T-0027]] *(no ADR needed — the contract already modeled `Partition`/`PartitionScheme`; honored it like S1/S2. Verified live: disjoint key-shards over Postgres.)*
- [x] `SyncMode::Cdc` via Postgres logical replication, LSN resume across runs. — S4 / [[WEIR-T-0028]] *(verified live: `test_decoding` slot, LSN/slot resume in `StreamState.opaque`)*
- [x] [[WEIR-S-0004]]/[[WEIR-S-0006]] updated to mark these dimensions implemented. *(2026-06-20)*
