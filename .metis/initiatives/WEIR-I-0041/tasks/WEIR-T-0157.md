---
id: snowflake-destination-sql-api-on
level: task
title: "Snowflake destination (SQL API) on the shared declarative runtime"
short_code: "WEIR-T-0157"
created_at: 2026-07-15T02:08:41.409192+00:00
updated_at: 2026-07-15T23:07:28.025504+00:00
parent: WEIR-I-0041
blocked_by: [WEIR-T-0156]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Snowflake destination (SQL API) on the shared declarative runtime

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

The warehouse side of five of the six client pipelines: a Snowflake **destination** that writes record batches
via the SQL API (`POST /api/v2/statements`) — table ensure/create, batched multi-row `INSERT`, idempotent
replay — riding `rest-dest` ([[WEIR-A-0034]]) if it fits, else a thin dedicated dest guest.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] A connection can target Snowflake (account/db/schema/warehouse from the `snowflake` bundle) and land
      records in a table matching the stream schema; append and upsert-by-key write modes ([[WEIR-I-0028]]).
- [ ] Batched INSERTs (multi-row VALUES or bind arrays) — not one request per record; async statement polling
      (`202` + statement handle) handled.
- [ ] Replay-safe: re-delivering a checkpointed batch does not duplicate rows (upsert path uses `MERGE`).
- [ ] Live test in the authed suite (`angreal test connectors-live`) writes + reads back a fixture batch against
      the trial account.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Spike first: can `rest-dest`'s manifest shape express statement-building (SQL text with bind params from record
fields)? If the templating can't express multi-row binds cleanly, promote to a small compiled dest guest that
reuses the host HTTP egress — decision recorded here for [[WEIR-I-0041]] design ratification. Type mapping:
stream schema → Snowflake types via the [[WEIR-I-0025]] typed-schema model.

### Dependencies
[[WEIR-T-0156]] (key-pair JWT auth). Informs [[WEIR-T-0158]] (shares statement plumbing).

### Risk Considerations
SQL API async statement semantics (202 + polling) don't exist in rest-dest today; that's the likeliest forcing
function toward the compiled path.

## Status Updates **[REQUIRED]**

### 2026-07-15 — spike decided: thin compiled dest guest

**Spike answer (rest-dest doesn't fit).** `rest-dest`'s write model is one HTTP request per record with
declarative body shaping (`field_map`/`body_wrap`, `render_path`) — read in full. Snowflake needs the opposite
shape: dynamically-sized multi-row SQL text scaled to the batch (`INSERT … VALUES (?,?),(?,?),…` + a numbered
bindings map), `CREATE TABLE IF NOT EXISTS` from an inferred schema, `MERGE` for upsert, and `202 →
statementHandle → poll` async semantics. That's statement **compilation**, not body templating. Decision:
**new compiled dest guest `crates/connectors/snowflake-dest`** (the task's pre-authorized fallback), keeping
the host seams: key-pair JWT auth injected host-side ([[WEIR-T-0156]]), `wasi:http` egress, same
retry/backoff conventions. Scaffolded via `angreal connectors new snowflake-dest`.

**v1 shape (mirrors `postgres` dest conventions where they exist):**
- Config: `account`, `database`, `schema`, `warehouse`, optional `role`/`table` (default = stream name),
  `batch_rows` (rows per statement, default 500), `max_retries`. URL
  `https://<account>.snowflakecomputing.com/api/v2/statements?async=false`.
- Columns + types inferred from the first record (string→VARCHAR, int→NUMBER, float→DOUBLE, bool→BOOLEAN;
  nested object/array → VARCHAR JSON text in v1 — VARIANT via `PARSE_JSON` noted as follow-up).
- Append: chunked multi-row `INSERT` with sequential bindings. Upsert: `MERGE INTO t USING (SELECT … FROM
  VALUES …)` keyed on `WriteMode::Upsert.business_keys` — idempotent on replay. Changes: Insert/Update →
  MERGE; Delete → keyed `DELETE`.
- `202` responses poll `GET /api/v2/statements/<handle>` with backoff; 429/5xx retry as usual; SQL 4xx fails
  the statement (schema-level error → sync failure, not silent dead-letters).

### 2026-07-15 — implemented; mock-verified green; live test staged

- **Hoist pattern honored ([[WEIR-I-0032]] context):** all statement compilation is pure + unit-tested in
  `weir_connector_types::snowflake` (14/14 incl. 6 new: type inference, CREATE/INSERT, keyed MERGE + unknown-key
  rejection, DELETE-USING, row-major bindings, dedup-last-wins). The guest
  (`crates/connectors/snowflake-dest`) is orchestration + HTTP only.
- Guest handles Rows (append=INSERT / upsert=MERGE with in-batch key dedup), Changes (ordered runs: Insert/
  Update→MERGE, Delete→DELETE-by-key), Overwrite (TRUNCATE once), chunking (`batch_rows` default 500), 202→poll,
  429/5xx retry. `base_url` override for PrivateLink/tests. Statement errors fail the sync (schema-level, not
  per-record).
- Engine tests (`wasm_snowflake_dest_engine`, 3/3 through the real wasm guest + mock SQL API): batched INSERT
  (one statement for 3 records, session context + row-major binds asserted), keyed MERGE with dedup (last dup
  wins, verified via bindings), 202→handle→poll settlement.
- **Live test staged:** `snowflake_dest_writes_and_reads_back_live` in `manifest_corpus` (rides the
  connectors-live SOPS flow; skips without the bundle) — writes 2 keyed rows through the guest with the
  key-pair credential, **replays the batch**, reads back via the rest runtime (SQL API SELECT, POST-body) and
  asserts exactly 2 rows (MERGE idempotency + the T-0156 fingerprint sanity in one shot). Runs green the moment
  `secrets/snowflake.enc.json` exists ([[WEIR-S-0018]] §2); `secrets/snowflake.example.json` documents the shape.
- Plumbing: staged in `scripts/stage-connectors.sh` (distribution image); corpus 34/34; fmt+clippy clean.

**AC status:** 1–3 fully verified against the mock API through the real guest; AC-4 (live) is implemented and
gated only on the trial-account bundle — the human-side provisioning step. T-0158 can reuse
`weir_connector_types::snowflake` + the read-back pattern directly.
