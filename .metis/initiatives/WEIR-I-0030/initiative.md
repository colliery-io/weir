---
id: deep-persistence-a-connection-work
level: initiative
title: "Deep persistence — a connection/work-unit store seam"
short_code: "WEIR-I-0030"
created_at: 2026-07-08T14:58:55.694276+00:00
updated_at: 2026-07-08T23:53:24.668093+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: deep-persistence-a-connection-work
---

# Deep persistence — a connection/work-unit store seam Initiative

## Context

The connection and work-unit persistence hand-spells the row↔struct mapping as a positional tuple at every call site. For `connections` the same 12-column ordering is transcribed **six times** — `ConnTuple`, the `add_connection` update `.set((…))`, the insert `.values((…))`, the `list`/`get` `.select((…))`, and `row_to_conn`'s destructure+rebuild — plus the `Connection` struct, `schema.rs`, and the `weir-api` DTO's two conversions. The orchestrator repeats it for `work_units` (`SpecTuple`, plan insert, load select+destructure+rebuild, a separate `RunTuple`). Portable upsert is update-then-insert (MultiBackend has no `on_conflict`), doubling the write sites.

**Measured cost:** adding one column touches ~10 edit sites, all positional — a mis-ordered or type-compatible column (e.g. the `every_secs` f32↔f64 juggling) is a silent corruption caught only when the tuple arity finally disagrees. This friction was paid twice in a row — [[WEIR-I-0028]] (sync/write modes) and [[WEIR-I-0029]] (per-side config) each meant a dozen coordinated edits. There is no repository **seam**: the mapping has no home, and every store test must stand up SQLite because nothing above the store can be faked.

Surfaced by the 2026-07-08 architecture review as a **Strong** deepening candidate — it passes the deletion test in the good direction (a single owner *concentrates* the mapping and deletes the transcriptions).

## Goals & Non-Goals

**Goals:**
- A **store module** whose interface is `get / list / upsert(Connection)` (and the work-unit equivalent), hiding the column plumbing from the app and orchestrator.
- Carry the row↔struct mapping in diesel derives (`Queryable` / `Selectable` / `Insertable`) on named row structs — a new field is one struct edit, checked by the compiler.
- Delete `ConnTuple`, `row_to_conn`, `SpecTuple`, `RunTuple` and the duplicated `.select`/`.values` column lists outright.
- Land the concrete `store` module as the natural **future seam** (a narrow `get/list/upsert` surface) — without building a trait/fake speculatively; that waits for a second consumer (one adapter = a hypothetical seam).

**Non-Goals:**
- Changing any column or persisted semantics — a pure refactor; same rows, same SQL behaviour.
- Replacing diesel-dualdb or the portable update-then-insert upsert strategy.
- The work-unit queue/lease/claim mechanics — only the row mapping, not the locking.

## Detailed Design

*(Proposed — for sign-off before decomposition.)*

- **Row structs.** `ConnectionRow` / `WorkUnitRow` / `RunRow` with `#[derive(Queryable, Selectable, Insertable)]` + `#[diesel(table_name = …)]`, field order/type matched to `schema.rs`. `Selectable` gives `.select(ConnectionRow::as_select())`; `Insertable` gives `.values(&row)`. The JSON-column (de)serialization and mode parsing live in one `From<&Connection> for ConnectionRow` + `TryFrom<ConnectionRow> for Connection` pair, replacing the scattered `json()`/`from_json()` and positional destructures.
- **Store module.** `weir-app::store` + `weir-orchestrator::store` expose `get/list/upsert` and `enqueue/load` over the rows. The update-then-insert upsert is expressed once, against the row.
- **Dualdb validation (done — 2026-07-08).** diesel-dualdb builds on Diesel's `#[derive(MultiConnection)]` + a `MultiBackend` bridge, and its *own* test suite derives `#[derive(Queryable, Insertable)]` on a row struct — so **load + insert derives are proven** over `DualConnection`. Only `Selectable`/`as_select()` is unverified; the fallback is a single shared `SELECT_COLUMNS` const in the store + `.load::<Row>()` (Queryable alone suffices), so the scatter collapses either way. `on_conflict` stays unsupported → the update-then-insert upsert is retained.
- **Seam (concrete now, trait deferred).** The concrete `store` module *is* the seam-in-waiting — a narrow `get/list/upsert` surface, trivially trait-ifiable later. Per the *one adapter = hypothetical seam* rule (and the Alternatives below), we do **not** introduce a trait + in-memory fake now (one consumer, one backend-pair); that's the documented next step when a second impl appears. The immediate win is killing the scatter.

## Alternatives Considered

- **Keep the tuples, just add a test.** Doesn't touch the ~10-site fan-out or the silent-ordering footgun. Rejected.
- **A macro that expands the column list.** Removes duplication but keeps positional coupling and adds a bespoke macro to own; diesel's own derives are the idiomatic, compiler-checked path. Rejected.
- **A full repository trait now.** Premature — one backend-pair, one consumer each. Start with row structs + a concrete store; extract a trait only if a second implementation appears (one adapter = a hypothetical seam).

## Implementation Plan

Proposed decomposition (**2 tasks**) — for sign-off:
1. **weir-app connection store.** `ConnectionRow` (`#[derive(Queryable, Selectable, Insertable)]`) + `From<&Connection>` / `TryFrom<ConnectionRow>` (JSON (de)ser, mode parse, f32↔f64 — once); a `weir-app::store` module (`get`/`list`/`upsert`); rewire `add_connection` (update-then-insert), `list_connections`, `get_connection`; delete `ConnTuple`, `row_to_conn`, the app `RunTuple`. Prove the derives round-trip a `connections` row on **both** backends in `dual_backend.rs` (this folds the `Selectable` check). If `as_select()` doesn't compose, fall back to a shared `SELECT_COLUMNS` const.
2. **orchestrator work-unit store.** `WorkSpecRow` + `RunRow` + a `weir-orchestrator::store` module; rewire `plan`/`load`/`recent_runs` and the inline `.select`/`.values` tuple sites; delete `SpecTuple`/`RunTuple`. Full workspace + clippy + full suite green.

**Exit criteria:** adding a column is demonstrably 1–2 edits (one struct field); every positional row-tuple (`ConnTuple`/`SpecTuple`/`RunTuple`/`row_to_conn`) is gone; the row↔struct mapping lives in one `From`/`TryFrom` per table; all suites + clippy clean. The `store` module is left as the concrete future seam (trait + fake deferred).
