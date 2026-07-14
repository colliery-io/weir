---
id: per-connection-sync-write-modes
level: initiative
title: "Per-connection sync + write modes (CDC exposure)"
short_code: "WEIR-I-0028"
created_at: 2026-07-08T03:01:53.212424+00:00
updated_at: 2026-07-08T03:29:59.245125+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: per-connection-sync-write-modes
---

# Per-connection sync + write modes (CDC exposure)

## Context

The docs pass ([[WEIR-I-0027]]) surfaced a real gap: **CDC + write modes are built and proven at the engine layer
but unreachable by an operator.** `App::work_spec` hardcodes `sync_mode: FullRefresh` and `write_mode: Append`, so
a connection created via the CLI/API can never run Incremental or CDC, nor Upsert/Overwrite — even though the
engine, the Postgres CDC source, and the delete-propagating destinations ([[WEIR-I-0026]]) fully support them
(the fidelity harness drives a `ConfiguredStream` directly). This closes that gap: **expose the modes on the
connection.**

`on_delete` / tombstone already flow through the connector **config** (the Postgres dest reads them from its
`cfg.json`), so they need no new plumbing — only documentation. The gap is `sync_mode`, `write_mode`,
`business_keys`, and `cursor_field`.

## Goals & Non-Goals

**Goals:**
- A connection carries a **sync mode** (full-refresh / incremental / cdc), a **write mode** (append / upsert /
  overwrite) with **business keys**, and a **cursor field** (for incremental) — persisted and honored.
- `work_spec` builds the `ConfiguredStream` from these instead of hardcoding.
- Settable via the CLI (`connection add`) and the API (`POST /connections`), with validation.
- An end-to-end test: a CDC delete propagated **through a connection** (not just the engine harness).

**Non-Goals:**
- New engine/connector behaviour — the execution already supports every mode; this is exposure + wiring.
- Auto-detecting a source's best mode; the operator chooses (bounded by what the connector supports).

## Proposed design

- **Model**: add `sync_mode` (TEXT, default `full_refresh`), `write_mode` (TEXT, default `append`),
  `business_keys` (TEXT — JSON array, nullable), `cursor_field` (TEXT, nullable) to the `connections` table
  (baseline edit + regen) and the `Connection` struct.
- **Wiring**: `work_spec` parses them into `SyncMode` / `WriteMode { business_keys }` / `cursor_field` on the
  `ConfiguredStream`. Backward-compatible defaults reproduce today's behaviour.
- **Validation** (at `add_connection`): `upsert` requires non-empty `business_keys`; `incremental` requires a
  `cursor_field`; unknown mode strings are rejected. (CDC-source support is the connector's concern at run time.)
- **Surface**: CLI `--sync-mode` / `--write-mode` / `--business-keys` (comma) / `--cursor-field`; API DTO fields
  with the same defaults; round-trip on list.
- **Docs**: drop the "not yet surfaced" caveats in `guides/cdc-deletes.md` + `reference/connection-config.md`.

### Design decisions (2026-07-08, approved)

- **Two tasks**: (T-a) model + persistence + `work_spec` wiring + validation + CLI/API surface; (T-b) end-to-end
  CDC-through-a-connection test + docs-caveat removal.
- **Business keys**: a comma-separated `--business-keys id,tenant` CLI flag (API takes a JSON array).
- Backward-compatible defaults (`full_refresh` / `append`) reproduce today's behaviour exactly.

## Decomposition

- **T-a — Model + wiring + surface:** `sync_mode`/`write_mode`/`business_keys`/`cursor_field` on the `connections`
  table (regen) + the `Connection` struct; `work_spec` parses them into the `ConfiguredStream`; validation in
  `add_connection` (upsert⇒keys, incremental⇒cursor, unknown⇒reject); CLI flags + API DTO fields; round-trip on
  list. Workspace + clippy clean.
- **T-b — End-to-end + docs:** an integration test that a CDC delete propagates **through a connection** to a
  destination (mirroring the T-0117 harness but via `App`/connection, not a raw `ConfiguredStream`); remove the
  "not yet surfaced" caveats from `guides/cdc-deletes.md` + `reference/connection-config.md`. Closes the initiative.

## Exit Criteria (draft)

- [ ] A connection persists + honors sync mode, write mode, business keys, cursor field; `work_spec` no longer
  hardcodes.
- [ ] Settable + validated via CLI + API; round-trips on list.
- [ ] An end-to-end test lands a CDC delete at a destination **through a connection**; workspace + clippy clean.
- [ ] The docs caveats are removed.
