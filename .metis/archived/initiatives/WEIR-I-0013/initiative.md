---
id: portable-postgres-control-plane
level: initiative
title: "Portable Postgres control-plane store (diesel-dualdb DSL conversion)"
short_code: "WEIR-I-0013"
created_at: 2026-06-25T03:07:56.173553+00:00
updated_at: 2026-06-25T12:06:53.445423+00:00
parent:
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: portable-postgres-control-plane
---

# Portable Postgres control-plane store (diesel-dualdb DSL conversion)

## Context **[REQUIRED]**

weir's control-plane store ([[WEIR-A-0009]]) was built v0-SQLite-only as a deliberate
shortcut: every crate talks to the shared `diesel-dualdb` pool through **raw
`diesel::sql_query` strings written in SQLite dialect** — `BLOB` columns, `?`
placeholders, `INTEGER PRIMARY KEY` rowid autoincrement. `weir-engine/src/lib.rs:10`
says it outright: *"Raw SQL uses SQLite placeholders; the portable-DSL / Postgres path
is later."* **This is "later."**

The gap surfaced the moment we pointed the demo at Postgres (the production SoR): weir
crashes at schema creation with `type "blob" does not exist`. Postgres connectivity
works (libpq opens the connection); the SQL itself is SQLite-only. `diesel-dualdb`
already solves this — it makes the **typed query builder** portable across Pg+SQLite via
`MultiBackend`, ships portable `sql_types` (`Bytes`→`bytea`/`BLOB`, `Timestamp`→
`timestamptz`/`TEXT`, `Json`, `Uuid`), generates per-backend migrations + a portable
`schema.rs` from one logical DDL (the `diesel-dualdb-schema` CLI), and offers
`DualConnection::dispatch` for the rare genuinely-divergent statement. We just have to
**use** it instead of hand-writing SQLite SQL.

Scope of the shortcut, measured: **~52 raw `sql_query` sites / 49 `?`-placeholder
queries** — weir-engine 13, weir-app 13, weir-orchestrator 26 — and **zero `table!`
schemas** in the tree today.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A fresh `weir` comes up and runs end-to-end against **Postgres** (the SoR) — schema
  creation through orchestration — with the existing **SQLite** suite still green.
- All control-plane SQL goes through `diesel-dualdb`'s portable surface: logical-DDL
  migrations → generated per-backend migrations + `schema.rs`, the typed query builder
  for DML, `dispatch` only where a statement genuinely can't be shared.
- The compose demo serves on the Postgres SoR (closes the gap that opened this).
- Both-backend coverage locked in via `#[diesel_dualdb::test(pg, sqlite)]`.

**Non-Goals:**
- No new control-plane features or schema changes beyond what portability requires
  (a faithful conversion, not a redesign).
- Not touching `diesel-dualdb` itself (it's the user's crate; we consume it).
- Connector-level Postgres (the `postgres` *connector* doing data TCP) is already done
  ([[WEIR-A-0026]] / WEIR-T-0046) — out of scope; this is the *store*.

## Detailed Design **[REQUIRED]**

Per the diesel-dualdb getting-started workflow:

1. **Logical schema.** One logical-DDL migration set (logical types: `BYTES`, `TIMESTAMP`,
   `JSON`, `UUID`, portable autoincrement) covering every control-plane table:
   weir-engine (`dead_letters`, `stream_state`, `outbox`, `run_logs`), weir-app
   (`connectors`, `connections`), weir-orchestrator (work-unit queue). Decide layout:
   one shared generated `schema.rs` vs per-crate `table!` modules over the shared DB.
2. **Codegen.** `diesel-dualdb-schema <migrations> <generated>` → per-backend migrations
   (`migrations-postgres/…`, `migrations-sqlite/…`) + portable `schema.rs`. Generated
   output is committed; wire an `angreal` task so it's reproducible.
3. **Runtime migration runner.** Replace each crate's `migrate()`/`open()` DDL with a
   dispatched runner: `match &*conn { Pg(_) => include_str!(pg up.sql), Sqlite(_) =>
   include_str!(sqlite up.sql) } ; conn.batch_execute(up)`.
4. **DML conversion.** Rewrite the 52 `sql_query` sites onto the typed query builder
   against `&mut DualConnection` using the generated schema + portable types. `dispatch`
   only where unavoidable (e.g. upsert/`on_conflict`, which MultiBackend doesn't carry).
5. **Verify on both backends** at every slice — SQLite (existing suite) + Postgres (the
   `angreal integration` compose / `DUALDB_PG_URL`).

Sequencing is **engine → app → orchestrator** so the smallest, most-isolated store
(engine) proves the whole pattern (codegen + runner + typed builder + dual-backend test)
before the larger surfaces adopt it.

## Alternatives Considered **[REQUIRED]**

- **Backend-aware raw DDL/queries (hand-branch `BLOB`/`BYTEA`, `?`/`$n`).** Rejected:
  re-implements by hand exactly what diesel-dualdb generates, across 52 sites, with no
  type safety and permanent drift risk. The user explicitly redirected away from this.
- **Postgres-only store (drop SQLite).** Rejected: SQLite is the zero-config dev/demo
  path and the entire existing test suite; dual-backend is the point of the chosen store.
- **Keep store on SQLite, Postgres only as a connector.** Rejected as the end state (the
  SoR must be Postgres) — but it IS the interim demo posture until this lands.

## Implementation Plan **[REQUIRED]**

Slices (one task each), each green on **both** backends before the next starts:

- **S1 — Foundation + weir-engine proof.** Codegen harness (logical migrations,
  `diesel-dualdb-schema`, generated `schema.rs` + per-backend migrations, angreal task),
  dispatched migration runner, and convert weir-engine's `Store` (4 tables, 13 queries).
  Exit: engine comes up + round-trips on Postgres; SQLite suite green; one
  `#[diesel_dualdb::test(pg, sqlite)]` proving the pattern.
- **S2 — weir-app.** Catalog/connections schema + 13 queries onto the portable surface.
- **S3 — weir-orchestrator.** Work-unit queue schema + 26 queries (largest surface).
- **S4 — Close.** A fresh `weir` migrates + serves on Postgres end-to-end; compose demo
  flipped back to the Postgres SoR and verified; dual-backend tests across the store.
