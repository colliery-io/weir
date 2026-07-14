---
id: per-side-connector-config-source
level: initiative
title: "Per-side connector config (source/dest split)"
short_code: "WEIR-I-0029"
created_at: 2026-07-08T10:53:32.791815+00:00
updated_at: 2026-07-08T11:31:11.075750+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: per-side-connector-config-source
---

# Per-side connector config (source/dest split)

## Context

A connection carries **one `config` JSON**, and the executor hands it to **both** the source and destination
connectors (`weir-orchestrator` `execute`: `spec.source.resolve(&spec.config)` and
`spec.dest.resolve(&spec.config)`). When both sides read the same key — e.g. Postgres's `url`/`table` on a
`postgres → postgres` connection — they collide: source table == dest table, a loop. This is the constraint
called out closing [[WEIR-I-0028]]. Give the two sides **their own config** so a source and destination can be
configured independently.

## Goals & Non-Goals

**Goals:**
- The source and destination of a connection resolve with **independent config**.
- Backward-compatible + ergonomic: a shared value (a DB `url`) is written once, not duplicated.
- Threaded end to end: connection model → `work_spec` → `WorkSpec` → `work_units` → the executor's two `resolve`s.
- Settable via CLI + API; the manifest-baking (`resolve_manifest_source`/`_dest`) bakes into the correct side.
- Removes the shared-config caveat from the CDC how-to; a `postgres → postgres` (different tables) connection works.

**Non-Goals:**
- Reworking the connector contract or the config schema of any connector.
- Secrets handling changes (the per-side split composes with the existing host-side credential path).

## Design decisions (2026-07-08, approved)

- **Model: two full independent configs.** Replace the connection's single `config` with **`source_config`** and
  **`dest_config`**, each a complete JSON. The stored + threaded model is the two configs; a CLI/API **`config`
  convenience** sets both when the per-side ones aren't given (so `--config '{…}'` keeps working, and a
  single-config connection is unchanged). Explicit `--source-config` / `--dest-config` override each side.
- **Two tasks**: (T-a) internal split; (T-b) surface + test + docs.

### Threading

- **Model**: `connections.source_config` / `dest_config` (TEXT, default `{}`) replace `config`; the `Connection`
  struct carries both.
- **`work_spec`**: `resolve_manifest_source` bakes into `source_config`, `resolve_manifest_dest` into
  `dest_config` (no longer chained into one blob). The connection-level in-flight `__mapping` is extracted from
  the configs and **stripped from both** so no guest sees it.
- **`WorkSpec` + `work_units`**: replace `config` with `source_config` + `dest_config` (pre-release, baseline
  schema edit + regen — no deployed data to migrate).
- **Executor**: `spec.source.resolve(&spec.source_config)`, `spec.dest.resolve(&spec.dest_config)`.
- **Surface**: CLI `--config` (both) + `--source-config` / `--dest-config` (override); API DTO gains the two
  fields, with `config` as the both-sides convenience on create.

## Decomposition

- **T-a — Internal split:** model (`connections` + `Connection`), `work_spec` (per-side configs + manifest-bake
  routing + `__mapping` strip), `WorkSpec` + `work_units` (regen), executor's two `resolve`s. Unit test that
  `work_spec` yields the right per-side config. Workspace + clippy clean.
- **T-b — Surface + test + docs:** CLI `--config` / `--source-config` / `--dest-config`; API DTO; a test proving
  source + dest receive **different** config (e.g. pg→pg different tables, or asserting the two resolved configs);
  drop the shared-config caveat from `guides/cdc-deletes.md` + `reference/connection-config.md`. Closes it.

## Exit Criteria (draft)

- [ ] Source + dest resolve with independent config; `merge(base, override)` per side; defaults reproduce today.
- [ ] Threaded through `work_spec` → `WorkSpec` → `work_units` → executor; manifest bakes hit the right side.
- [ ] Settable via CLI + API; a test proves the two sides receive different config (e.g. pg→pg different tables).
- [ ] The shared-config caveat is removed from `guides/cdc-deletes.md`; workspace + clippy clean.
