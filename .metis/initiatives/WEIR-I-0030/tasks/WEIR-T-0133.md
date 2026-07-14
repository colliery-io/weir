---
id: weir-app-connection-store
level: task
title: "weir-app connection store — ConnectionRow + store module"
short_code: "WEIR-T-0133"
created_at: 2026-07-08T17:27:35.762693+00:00
updated_at: 2026-07-08T23:03:07.953562+00:00
parent: WEIR-I-0030
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0030
---

# weir-app connection store — ConnectionRow + store module

## Parent Initiative

[[WEIR-I-0030]] — the first of two: deepen `connections` persistence into a row struct + store module.

## Objective

Replace the positional `connections` row-tuple plumbing in `weir-app` with a `ConnectionRow` (diesel derives) and
a concrete `store` module, so the row↔struct mapping lives in one place and adding a column is one struct-field
edit. Delete `ConnTuple`, `row_to_conn`, and the app-side `RunTuple`.

## Reference

- `crates/weir-app/src/lib.rs` — `ConnTuple` (:207), `RunTuple` (:224), `add_connection` (:257, update-then-insert
  at :284/:303), `list_connections`/`get_connection` selects (~:389/:417), `row_to_conn` (~:1587).
- `crates/weir-schema/schema/generated/schema.rs` — `connections (tenant_id, name)` (:75).
- `crates/weir-schema/tests/dual_backend.rs` — `app_tables_round_trip` (:133) is where the both-backends proof lands.
- Validation (initiative): diesel-dualdb derives `Queryable`/`Insertable` in its own tests → proven; `as_select()`
  is the one unknown, fallback = a shared `SELECT_COLUMNS` const.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `ConnectionRow` with `#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable)]` — the full
  clean design; `from_resolved` + `into_connection` own the JSON (de)ser, mode parsing, and f32↔f64 **once**.
- [x] `weir-app::store` (a new `store.rs`) exposes `get`/`list`/`upsert` + `run_feed` over the rows; `add_connection`,
  `list_connections`, `get_connection`, `recent_runs` call it.
- [x] `ConnTuple`, `row_to_conn`, and the app `RunTuple` **deleted**; `lib.rs` −140 lines (33 added, 173 removed).
  No fallback needed — `as_select()` **and** `AsChangeset`/`.set(row)` compose over `DualConnection`.
- [x] `dual_backend.rs::connection_row_derives_round_trip` proves insert→select→AsChangeset-update round-trips on
  **both** SQLite and **live Postgres** (`:5433`).
- [x] Column-add is one struct field by design (`AsChangeset` + `Insertable` + `Selectable` all read the struct —
  no `.set`/`.values`/`.select`/tuple lists to touch). `weir-app` + `weir-api` build; 23 lib + all integration
  tests green; workspace clippy `-D warnings` green.

## Implementation Notes

### Technical Approach
`ConnectionRow` mirrors the `connections` columns (`tenant_id, name, source_ref, dest_ref, stream, source_config,
dest_config, every_secs: Option<f32>, cron, sync_mode, write_mode, business_keys, cursor_field`). `Insertable` over
the row for the insert; the upsert stays update-then-insert (no `on_conflict` on MultiBackend). Reads use
`.select(ConnectionRow::as_select())` → `.load()`, with a shared `SELECT_COLUMNS` const as the fallback if
`Selectable` doesn't compose over `DualConnection`. The public `Connection` struct is unchanged — `ConnectionRow` is
its storage-facing twin; `TryFrom` runs `parse_sync_mode`/`parse_write_mode`, the `business_keys` JSON, and f32→f64.
The `weir-api` DTO sits above the store and is untouched.

### Dependencies
None — first task of [[WEIR-I-0030]]. [[WEIR-T-0134]] (orchestrator) follows and reuses the pattern.

### Risk Considerations
Only `as_select()` composing over MultiBackend is unproven; `Queryable`/`Insertable` are proven by diesel-dualdb's
own tests. Fallback (shared `SELECT_COLUMNS`) still collapses the scatter, so the task can't be blocked by it.

## Status Updates

### 2026-07-08 — done

`store.rs` owns the `connections` mapping: one `ConnectionRow` with the full derive set. The initiative's residual
risk (`Selectable` over `DualConnection`) resolved in the good direction — not only `as_select()` but `AsChangeset`
`.set(row)` compose, so the design is maximally clean: **a new column is one struct field**, no `.select`/`.values`/
`.set`/tuple lists anywhere. Proven on both backends (SQLite + live Postgres) in `dual_backend.rs`.

**Two caveats worth recording:**
1. **rustfmt version drift in this dev env.** The toolchain here is `rust 1.93.0 / rustfmt 1.8.0` (2026-01-19) —
   older than the one the repo was formatted under, so `cargo fmt --check` diffs the *entire* workspace. An early
   `cargo fmt -p weir-app` reformatted the whole crate (~1149 spurious lines); I reverted it and hand-applied the
   rewire to match surrounding style. The lib.rs diff is now logical-only (−140). **fmt must be validated under the
   repo's pinned toolchain**, not this one.
2. **Clippy gate.** `angreal check clippy` is `-D warnings` workspace-wide and was red on pre-existing lints a
   stricter clippy now fires: two `collapsible_if` (weir-engine, weir-soak — fixed with let-chains) and
   `large_enum_variant` on the CLI command enums (I enlarged `ConnAction::Add` in [[WEIR-I-0029]]; scoped
   `#[allow]` since a command enum is parsed once). Workspace clippy is green again.

Complete — [[WEIR-T-0134]] (orchestrator) is next, reusing this exact pattern.
