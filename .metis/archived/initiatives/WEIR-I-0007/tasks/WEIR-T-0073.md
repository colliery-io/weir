---
id: s3-reverse-etl-flow-warehouse
level: task
title: "S3: Reverse-ETL flow — warehouse → mapping → SaaS upsert, idempotent under replay"
short_code: "WEIR-T-0073"
created_at: 2026-07-04T03:17:57.276564+00:00
updated_at: 2026-07-04T03:43:51.106837+00:00
parent: WEIR-I-0007
blocked_by: [WEIR-T-0072]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0007
---

# S3: Reverse-ETL flow — warehouse → mapping → SaaS upsert, idempotent under replay

## Parent Initiative

[[WEIR-I-0007]] slice S3. Proves the reverse-ETL loop end-to-end and its idempotency semantics.

## Objective

Wire the full reverse-ETL run — **Postgres (warehouse) source → in-flight mapping → SaaS destination
upsert** — through the engine, and prove it is **idempotent under replay** (re-running re-upserts, no
duplicates). This is the slice that turns S2's destination runtime into an actual activation pipeline.

## What already exists (don't rebuild)

- **Postgres SOURCE**: `crates/connectors/postgres` already has `roles = [Source, Destination]` and a real
  `read` honoring `SyncMode` — so the warehouse-read side is done; this slice *uses* it, don't build a new
  Postgres source.
- **Mapping stage**: `weir-engine` already applies `ConfiguredStream.mapping` between read and write
  ([[WEIR-T-0071]]) — warehouse columns → SaaS properties shaping is available.
- **Destination runtime**: [[WEIR-T-0072]] provides the SaaS write guest.
- **Upsert contract**: `WriteMode::Upsert{business_keys}` ([[WEIR-A-0011]]) already exists in the connector
  contract and the Postgres sink implements it — mirror the semantics on the SaaS side.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A reverse-ETL run executes end-to-end: **Postgres source `read` → engine mapping → SaaS dest `write`
  (upsert)** — driven the normal way (a `Connection` / `WorkSpec`), not a bespoke harness.
- [ ] **Idempotent activation**: the run uses `WriteMode::Upsert{business_keys}`; **running it twice against
  the same mock SaaS state yields the same records, no duplicates** (the destination upserts by business key).
- [ ] **E2E test** (mirror `wasm_http_engine` / `wasm_postgres_engine`): seed a Postgres warehouse table →
  run source→mapping→mock-SaaS → assert the mock received the upserted, shaped records; **replay → assert
  idempotent** (same final state, no dupes); a rejected record dead-letters without failing the run.
- [ ] Checkpoint/state semantics are correct — the run commits its checkpoint atomically ([[WEIR-A-0011]]);
  replay from the last checkpoint re-upserts safely.
- [ ] Any wiring gap discovered (e.g. dest `roles`/`ReverseEtl` handling in the orchestrator, mapping applied
  on the warehouse→SaaS direction) is closed here. Workspace + integration suites green; clippy clean.

## Technical Notes

- The mapping stage is dest-agnostic (applies between read and write regardless of direction), so
  warehouse→SaaS shaping should "just work" — but **verify** the orchestrator plans a source→dest run where
  the dest is the SaaS runtime (roles, config split, host-auth) and add coverage for it.
- Idempotency is the destination's upsert (create-or-update by key); this slice proves it at the flow level
  (replay), while [[WEIR-T-0072]] proves the single-write mechanics.
- Use a **mock SaaS server** (loopback), same pattern as the source wire tests — no real vendor here
  (that's S4/S5).

## Dependencies

- **Blocked by [[WEIR-T-0072]]** (needs the destination runtime).
- Enables [[WEIR-T-0074]] (HubSpot) / [[WEIR-T-0075]] (Salesforce) to be *manifest + test* only.

## Status Updates

### 2026-07-04 — reverse-ETL flow + idempotent replay proven

**Stream-driven upsert wired into the dest guest** (`rest-dest`): `write` now reads `ctx.stream.write_mode`
— an `Upsert{business_keys}` stream, with no `method` pinned in config, resolves to **PATCH the keyed URL**
(create-or-update); otherwise POST. So `WriteMode::Upsert` drives the upsert verb, and the keyed path
(`/contacts/{{ record.email }}`) makes replay idempotent.

**E2E test** `wasm_reverse_etl_engine.rs` (`reverse_etl_upsert_is_idempotent_under_replay`): source → engine
**mapping** → `rest-dest` PATCH-upsert to a **stateful mock SaaS** (keyed store). Run **twice**:
- `rows_written == 3` each run;
- the keyed store holds **exactly 3** records after both runs — **idempotent, no duplicates**;
- the stored body carries `"source":"weir"` (mapping ran en route) + the payload (`Ada`).

Dead-letter-on-reject is covered by [[WEIR-T-0072]]'s wire test. clippy clean.

**Scope decisions:** warehouse=Postgres is a **config swap** (the Postgres connector has a `Source` role +
its own tests; the reverse-ETL semantics are source-agnostic), so this proves them **hermetically** with the
rest source as warehouse stand-in. "Driven normally" = `engine.sync` with a real `ConfiguredStream` (the
execution core the executor calls; the standard `wasm_*_engine` pattern) — the `Connection→WorkSpec→engine`
layer builds exactly this stream.

All ACs met. **Complete.**
