---
id: snowflake-source-sql-api-retl-feed
level: task
title: "Snowflake source (SQL API) — rETL feed"
short_code: "WEIR-T-0158"
created_at: 2026-07-15T02:08:49.294480+00:00
updated_at: 2026-07-15T23:27:36.365156+00:00
parent: WEIR-I-0041
blocked_by: [WEIR-T-0154, WEIR-T-0156]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Snowflake source (SQL API) — rETL feed

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

The source half of the rETL pipeline (Snowflake → HubSpot): read a table/query from Snowflake via the
SQL API as a declarative source stream, with incremental reads on a cursor column, feeding the existing
`hubspot-dest.yaml` upsert.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] A source stream is a table or SELECT (db/schema/warehouse from the `snowflake` bundle); result-set
      partitions/pages are consumed fully (SQL API result partitioning handled).
- [ ] Incremental mode on a cursor column (e.g. `UPDATED_AT`) checkpoints and resumes correctly.
- [ ] End-to-end live test: seeded Snowflake table → weir → HubSpot contact upsert, green in the authed suite.
- [ ] Statement plumbing shared with [[WEIR-T-0157]] rather than duplicated.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
POST-with-body ([[WEIR-T-0154]]) + keypair auth ([[WEIR-T-0156]]) makes this expressible as a `rest` manifest:
body carries the SQL + bind for the cursor lower bound; pagination maps to result partitions. If T-0157's spike
forced the compiled path, reuse that guest for the source direction instead.

### Dependencies
[[WEIR-T-0154]], [[WEIR-T-0156]]; shares design outcome with [[WEIR-T-0157]].

### Risk Considerations
SQL API result partitions are fetched by partition index with the same statement handle — confirm the manifest
pagination model can express "loop partitions until exhausted"; if not, that's a small runtime extension.

## Status Updates **[REQUIRED]**

### 2026-07-15 — design: source role on the compiled snowflake guest (rename from snowflake-dest)

**Why not a rest manifest (the original sketch):** the SQL API returns result rows as **arrays**
(`data: [[v1,v2],…]`) with column names off in `resultSetMetaData.rowType` — the rest guest would emit
array-records with no named fields, breaking `hubspot-dest`'s `field_map`, upsert keys, and cursor-field
extraction. Result **partitions** (GET same handle `?partition=N`) also don't map onto its pagination model.
Task notes pre-authorized reusing T-0157's compiled guest in this case.

**Decision:** rename `snowflake-dest` → `snowflake` (crate `weir-snowflake-wasm`, pkg `weir-snowflake-pkg`,
roles Source+Destination) — the `postgres` guest precedent (one crate, both roles). Strongest form of AC-4:
one guest, one statement/exec/poll plumbing. T-0157 touchpoints updated (engine test, staging, live test).

**Source v1:** config `query` (or `table`, default stream name → `SELECT *`); incremental wraps the base as
`SELECT * FROM (<base>) WHERE <cursor> > ? ORDER BY <cursor>` with the state cursor bound; array rows are
zipped with `rowType` names into JSON objects (**field names lowercased** — friendly to mapping/field_map);
all result partitions consumed (`GET /statements/<handle>?partition=N`); cursor advances on max; Records +
Checkpoint emitted. Pure pieces (`incremental_select`, `rows_to_objects`) join
`weir_connector_types::snowflake` with unit tests. `exec` now returns the response JSON (dest ignores it).

### 2026-07-15 — implemented + green; live rETL staged

- Rename landed: `crates/connectors/snowflake` (crate `weir-snowflake-wasm`, pkg `weir-snowflake-pkg`, spec
  `snowflake`, roles Source+Destination). T-0157 touchpoints updated: engine test file (now
  `wasm_snowflake_engine.rs`), `scripts/stage-connectors.sh`, the T-0157 live test.
- Pure builders +3 (`incremental_select`, `ordered_select`, `rows_to_objects` w/ lowercasing + non-array-row
  drop): connector-types 16/16.
- Engine test `snowflake_source_reads_partitions_and_resumes_incrementally` (through the real guest + mock SQL
  API): 2 result partitions consumed (inline + `?partition=1` fetch); first run is a full **ordered** read (no
  WHERE); second sync's statement carries `WHERE "UPDATED_AT" > ?` with **the checkpointed max cursor —
  sourced from partition 1's row — bound as param 1**. Engine suite 4/4; wasm_http 22/22; corpus 34/34;
  runtime 16/16; fmt+clippy clean.
- **AC-3 live test staged:** `snowflake_to_hubspot_retl_live` in the connectors-live suite — seeds the
  warehouse table through the guest's write side, engine-syncs the guest's source side into `rest-dest` baked
  from the vendored `hubspot-dest.yaml` (token host-injected, asserted stripped from guest config), runs
  **twice** (idempotent PATCH upsert). Skips until BOTH `snowflake` + `hubspot` bundles land;
  `secrets/hubspot.example.json` added.

**AC status:** 1, 2, 4 fully verified against mocks through the real guest; AC-3 implemented + self-arming on
the two bundles ([[WEIR-S-0018]] §2–3). Awaiting review.
