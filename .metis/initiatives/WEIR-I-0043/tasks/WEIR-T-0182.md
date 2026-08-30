---
id: postgres-destination-typed-columns
level: task
title: "Postgres destination typed columns — schema-driven DDL instead of JSONB blob"
short_code: "WEIR-T-0182"
created_at: 2026-08-29T14:26:46.233215+00:00
updated_at: 2026-08-29T20:22:30.159041+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Postgres destination typed columns — schema-driven DDL instead of JSONB blob

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Let the postgres destination land TYPED relational columns instead of a JSONB blob — the warehouse-user expectation: a synced `orders` table should have `id bigint, total numeric, created_at timestamptz`, not one `data jsonb` column.

## Approach

- Drive DDL from the stream's typed schema ([[WEIR-I-0025]] Arrow-typed schemas travel with the sync): CREATE TABLE with mapped Postgres types on first write; column mapping Arrow→PG (int64→bigint, utf8→text, timestamp→timestamptz, float64→double precision, bool→boolean, decimal→numeric; unmapped/complex types fall back to jsonb PER COLUMN, not per row).
- Write modes preserved: append/overwrite/upsert-by-business-keys all operate on the typed columns; upsert keys become a UNIQUE constraint.
- Schema evolution honors the [[WEIR-I-0025]] additive-vs-breaking rules: additive columns ALTER TABLE ADD; breaking changes surface as the existing evolution error, never silent data mangling.
- Back-compat: the JSONB behavior remains available (config flag, e.g. `typed_columns: false`) — existing pipelines keep working; decide the default honestly (typed on for new connections is the goal).
- Sequencing: after [[WEIR-T-0176]] (same crate, avoid conflicts); does not depend on TLS functionally.

## Acceptance Criteria **[REQUIRED]**

- [x] A synced stream lands as typed columns matching the source schema (docker-gated tests assert column names + PG types via information_schema, and values round-trip typed)
- [x] Upsert-by-keys and delete propagation ([[WEIR-I-0026]]) work against the typed table (typed CDC apply; the full cdc suite runs in typed mode)
- [x] Additive schema evolution ALTERs the table; breaking changes error and dead-letter the offending rows (never silent mangling — the host evolution policy remains the gate)
- [x] JSONB fallback flag works; unmapped shapes degrade to jsonb columns, documented; `angreal check all` + unit wall green

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + verified (ralph run); default = TYPED (Dylan's "typed on for new connections is the goal"; breaking-change called out in CHANGELOG under the v0-unstable policy).**

- **Schema source**: inferred per write via `weir_connector_types::StreamSchema::infer` — the SAME inference the host's [[WEIR-I-0025]] schema capture uses (the discover IPC seam carries nothing platform-wide, per the [[WEIR-T-0179]] finding). Type map: Integer→bigint, Float→double precision, Boolean→boolean, Timestamp→timestamptz, Str→text, Json (objects/arrays/mixed)→jsonb per COLUMN.
- **Write paths** (`crates/connectors/postgres`): append/overwrite/upsert all typed — `ensure_typed_table` CREATEs with the full column set (+ typed PRIMARY KEY for upsert; a key absent from every row lands text) then `ALTER TABLE ADD COLUMN IF NOT EXISTS` per field (the additive path; works on both new fields AND pre-default legacy tables). Upsert emits `ON CONFLICT (pk) DO UPDATE SET col = EXCLUDED.col`. Value rendering is kind-faithful; a kind/column mismatch errors in PG and dead-letters the row via the existing bisection. **CDC apply is typed too**: Insert/Update as typed single-row upserts, deletes/tombstones by key predicate (untyped literals cast to the typed keys). `typed_columns: false` restores the legacy JSONB layout everywhere.
- **Two real bugs surfaced by running the suite typed**: (1) the incremental cursor predicate `t.col::text > '<cur>'` compared LEXICOGRAPHICALLY while ordering natively — on a bigint column '2' > '12' re-delivered rows; fixed with an untyped literal (PG casts to the column type; text columns unchanged) — a piece of [[WEIR-I-0044]]'s cursor-correctness landed early. (2) Test-suite hygiene: layout transitions poison persisted tables — `clean_tables` drops each writing test's tables up front (8 call sites).
- **Tests**: 3 new docker-gated (typed columns land + values round-trip + additive ALTER; typed-PK upsert idempotent-by-key; `typed_columns:false` keeps `data jsonb`); the WHOLE suite runs in typed mode — **20/20 green** (regression + CDC/deletes + partitions + TLS gates + discover). `angreal check all` clean; unit wall 12/12.
- **Docs**: connection-config gains a "Typed columns" section; CHANGELOG Unreleased carries the breaking default flip + the TLS defaults + the cursor/SigV4/s3/discover fixes from this ralph set.
