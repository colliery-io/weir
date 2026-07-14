---
id: split-config-internally-model-work
level: task
title: "Split config internally (model → work_spec → executor)"
short_code: "WEIR-T-0131"
created_at: 2026-07-08T11:06:26.745682+00:00
updated_at: 2026-07-08T11:23:27.476885+00:00
parent: WEIR-I-0029
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0029
---

# Split config internally (model → work_spec → executor)

## Parent Initiative

[[WEIR-I-0029]] — the internal plumbing: two configs, one for each side.

## Objective

Replace the single connection/work `config` with **`source_config`** + **`dest_config`**, threaded from the model
through `work_spec`, `WorkSpec`, and `work_units`, so the executor resolves each connector with its own config.

## Reference

- `crates/weir-app/src/lib.rs` — `Connection` (`config`), `add_connection` insert/read, `work_spec` (builds one
  `Config`), `resolve_manifest_source`/`resolve_manifest_dest` (currently chain into one config), `extract_mapping`
  (pulls `__mapping` from config).
- `crates/weir-orchestrator/src/lib.rs` — `WorkSpec.config` (line ~197), enqueue/load (`work_units::config`), and
  `execute`: `spec.source.resolve(&spec.config)` / `spec.dest.resolve(&spec.config)` (line ~900).
- `crates/weir-schema/schema/migrations/0001_init/up.sql` — `connections.config`, `work_units.config` (+ regen).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `connections` + `work_units` replace `config` with `source_config`/`dest_config` (regen'd, both backends);
  `Connection` carries both.
- [x] `work_spec` builds per-side configs — `resolve_manifest_source`→`source_config`, `resolve_manifest_dest`→
  `dest_config`; `__mapping` extracted + stripped from both.
- [x] `WorkSpec` + `SpecTuple` carry both; enqueue + load persist both.
- [x] Executor resolves `spec.source.resolve(&spec.source_config)` + `spec.dest.resolve(&spec.dest_config)`.
- [x] Unit test `work_spec_splits_source_and_dest_config` (pg→pg different tables); every construction site
  updated. Workspace compiles; 23 weir-app lib + orchestrator suite + clippy clean.

## Status Updates

### 2026-07-08 — done (`7f054ba`)

The whole config path is now per-side. The executor resolves each connector with its own config; the manifest
bakes route to the correct side; `__mapping` is stripped from both. A large but mechanical sweep updated every
`Connection`/`WorkSpec` construction (app + orchestrator + api + cli + schema tests). Full workspace + all tests
compile; the split test + 23 lib tests + the orchestrator suite pass; clippy clean. **Complete** — CLI/API
per-side flags + the pg→pg proof + docs are [[WEIR-T-0132]].
