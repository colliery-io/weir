---
id: postgres-real-discover-via
level: task
title: "Postgres real discover() via information_schema"
short_code: "WEIR-T-0179"
created_at: 2026-08-29T14:26:34.102723+00:00
updated_at: 2026-08-29T19:58:49.734617+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# Postgres real discover() via information_schema

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Make the postgres connector's `discover()` introspect REAL tables instead of returning its hardcoded stub stream — today users hand-type table names blind, which is the single worst first-connection experience for the flagship database source.

## Approach

- Port the mssql connector's pattern (it already does real discovery): query `information_schema.tables` + `information_schema.columns` (+ `key_column_usage` for primary keys) over the existing wire client, map Postgres types onto the typed-schema model ([[WEIR-I-0025]] Arrow-typed schemas), and emit one `StreamInfo` per table with schema IPC, source-defined PKs, and supported sync modes (FullRefresh always; Incremental where a plausible cursor column exists; CDC where the table qualifies).
- Scope to the connection's configured database/schema (default `public`); system schemas excluded.
- Discovery runs through the same `PgConn` — after [[WEIR-T-0176]] it inherits TLS transparently, but this task does NOT depend on TLS and can land first against plaintext.

## Acceptance Criteria **[REQUIRED]**

*(Amended 2026-08-29 during implementation: the "typed, not blank IPC" clause was dropped as over-spec — nothing platform-wide consumes `StreamInfo.schema.ipc` (mssql ships it blank too; no host/UI decoder exists), and the platform's typed schemas ([[WEIR-I-0025]]) are captured at sync via `StreamSchema::infer`. Filling the IPC would require the arrow crate in the guest for bytes nobody reads.)*

- [x] `discover()` against the integration Postgres returns the real tables with source-defined primary keys; the stub stream is gone
- [x] The UI stream dropdown / `GET` catalog surfaces show the discovered tables end-to-end (no UI change — `discover_streams` maps Catalog→names and Error→AppError already)
- [x] System schemas excluded; configured schema respected; empty-schema case returns an empty catalog, not an error
- [x] Docker-gated integration tests assert a seeded table's name + composite PK (ordinal order) round-trip through discover; `angreal check all` + unit wall green

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + verified (ralph run).**

- `discover()` (`crates/connectors/postgres`) now runs one `information_schema.tables` query (BASE TABLE, configured schema only — new `schema` config key, default `public`, in config_schema) LEFT JOINed to a `table_constraints`/`key_column_usage` aggregate for primary keys in ordinal order; one `StreamInfo` per table with `namespace = schema`, `source_defined_primary_key`, and FullRefresh/Incremental/Cdc modes. The hardcoded `"table"` stub is gone.
- **Error honesty beyond the mssql pattern**: a connect/query failure returns `DiscoverOutcome::Error` (mssql silently returns an empty catalog); `App::discover_streams` already maps that to a config error, so the UI shows a reason instead of an empty dropdown. An unknown/empty schema is an empty catalog (distinct from failure).
- Tests (docker-gated, green): `pg_discover_lists_real_tables_with_pks` (seeded composite-PK table `disc_t (id, tag)` → discovered with `["id","tag"]` in ordinal order, namespace `public`, stub absent) and `pg_discover_empty_schema_and_error_cases` (unknown schema → empty catalog; dead server → `DiscoverOutcome::Error`). Full pg regression 11/11 unchanged after the change.
- TLS note: discovery rides the same `PgConn`, so it inherits [[WEIR-T-0176]]'s TLS transparently.
