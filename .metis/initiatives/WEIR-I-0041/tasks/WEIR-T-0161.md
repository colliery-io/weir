---
id: mssql-source-connector-compiled
level: task
title: "MSSQL source connector (compiled) + integration compose"
short_code: "WEIR-T-0161"
created_at: 2026-07-15T02:09:09.018710+00:00
updated_at: 2026-07-16T01:02:50.874073+00:00
parent: WEIR-I-0041
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# MSSQL source connector (compiled) + integration compose

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

A compiled MSSQL **source** connector (`crates/connectors/mssql`) in the mold of the `postgres` connector:
table discovery, full refresh, and cursor-column incremental — plus an `mssql` service in the integration
compose stack so the suite runs it like Postgres. Demo bar is batch parity, not CDC ([[WEIR-I-0041]] non-goal).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] New WASM guest scaffolded via `angreal connectors new`, on the canonical contract block
      ([[WEIR-T-0135]]); TDS transport works through the host socket egress (the [[WEIR-A-0039]]-style brokered
      path the resident ws work built — TDS is a raw TCP protocol, not HTTP).
- [ ] `discover` lists tables/columns with types mapped onto the typed schema model ([[WEIR-I-0025]]);
      full-refresh and cursor-incremental reads checkpoint/resume correctly.
- [ ] `mcr.microsoft.com/mssql/server:2022` added to the integration compose (`angreal integration up`) with a
      seeded demo database; integration tests green in `angreal test integration`.
- [ ] Pure SQL/row logic lives unit-testable (the `weir-connector-types` hoist pattern), per the constraint
      that `cdylib` guests can't `cargo test` ([[WEIR-I-0032]] context).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Candidate driver: `tiberius` (Rust TDS) — verify it compiles to the WASM target with the fidius socket
transport; if native-TLS bindings block the WASM build, that's the first thing to spike (plaintext/TDS-level
encryption-off is acceptable against the compose service for the demo). Mirror the `postgres` crate's module
layout so review is diff-shaped.

### Dependencies
None — parallel-safe from day one. Feeds [[WEIR-T-0162]].

### Risk Considerations
`tiberius`-on-WASM is the long pole; spike it first and fall back to a minimal hand-rolled TDS login+query
subset if the crate won't cross-compile (demo needs SELECT + metadata queries only).

## Status Updates **[REQUIRED]**

### 2026-07-16 — spike WON: tiberius over sync socket + block_on compiles for wasm

The long-pole risk is retired. `tiberius = { default-features = false, features = ["tds73"] }` (tokio + TLS
off) **compiles for wasm32-wasip2**, and its async `Client` drives over the blocking `fidius_guest::sockets::tcp`
socket via a poll-always-`Ready` `AsyncRead`/`AsyncWrite` adapter + `futures::executor::block_on` — nothing
ever returns Pending, so block_on completes in one poll cycle with a no-op waker (verified: full release build
green incl. `Client::connect` + `simple_query` + `into_first_result`). **This beats hand-rolling TDS** — we
get tiberius's COLMETADATA/ROW token parsing and typed `Row`/`ColumnData` for free.

**Plan (mirrors `postgres` + `snowflake` conventions):**
- Pure SQL builders in `weir_connector_types::mssql` (hoist for unit tests, [[WEIR-I-0032]]): discover query,
  full-refresh `SELECT`, cursor-incremental `SELECT … WHERE col > @p ORDER BY col`, MSSQL-type→`FieldType` map.
- Guest `crates/connectors/mssql`: `MssqlConn` (SyncSock + tiberius Client, `block_on` helper); `check`
  (connect), `discover` (INFORMATION_SCHEMA tables/columns → typed schemas), `read` (full-refresh +
  cursor-incremental, tiberius `Row` → JSON, cursor advances on max). Source-only (batch parity; CDC is the
  initiative non-goal). Capability `tcp`.
- Integration: `mcr.microsoft.com/mssql/server:2022` in the compose (encryption off; plaintext login is the
  pre-authorized path), seeded demo DB, `angreal test integration` coverage.

TLS note: tiberius `EncryptionLevel::NotSupported`; the compose image runs with Force Encryption off (default),
so the plaintext login the risk note allows is what runs. Removing the spike scaffold now.

### 2026-07-16 — implemented + verified LIVE against real SQL Server

Built as planned. **Ran the integration suite against a live `mcr.microsoft.com/mssql/server:2022` container
(3/3 green)** — not mocked:
- `mssql_wasm_discovers_tables`: discover lists the seeded `dbo.contacts` (INFORMATION_SCHEMA via FOR JSON).
- `mssql_wasm_full_refresh_reads_all_rows`: 3 seeded rows read through the guest → arrow-sink.
- `mssql_wasm_incremental_advances_and_resumes`: cursor on `updated_at` advances to the max; a resume from the
  checkpoint reads 0 new rows.

The block_on-over-sync-socket + tiberius design works end-to-end over real TDS. Reads use `FOR JSON PATH`
(server produces JSON — the postgres `row_to_json` analogue), so no per-type row decoding in the guest.
**One live-caught bug:** the discover query lacked `FOR JSON PATH`, so `query_json` got a bare string — fixed
(added it; unit-tested). Pure builders in `weir_connector_types::mssql` (20/20 connector-types incl. 4 mssql:
identifier quoting, full/incremental SELECT, type map, discover-is-JSON). fmt+clippy clean; container torn down.

Plumbing: `compose.yml` mssql + mssql-seed services (loopback, `WEIR_MSSQL_HOST_PORT` override,
`test-fixtures/mssql/seed.sql`); staged in `scripts/stage-connectors.sh`; integration task help updated; README.

**All ACs met.** AC-1 (scaffold + TDS over socket egress) ✓, AC-2 (discover + full/incremental checkpoint-
resume) ✓ live, AC-3 (compose service + seeded DB + green integration tests) ✓ live, AC-4 (hoisted unit-
testable SQL/type logic) ✓. Note the `postgres`-parity limitation carried over: incremental compares the cursor
as text (`CONVERT(nvarchar…) >`), fine for datetime/ISO cursors (the rETL pipeline), lexical for bare ints.
Awaiting review.
