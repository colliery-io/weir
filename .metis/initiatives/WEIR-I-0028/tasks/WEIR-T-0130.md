---
id: cdc-through-a-connection-e2e-docs
level: task
title: "CDC-through-a-connection e2e + docs"
short_code: "WEIR-T-0130"
created_at: 2026-07-08T03:03:55.702511+00:00
updated_at: 2026-07-08T03:29:54.717488+00:00
parent: WEIR-I-0028
blocked_by: [WEIR-T-0129]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0028
---

# CDC-through-a-connection e2e + docs

## Parent Initiative

[[WEIR-I-0028]] — prove it, then correct the docs. Closes the initiative.

## Objective

Prove a CDC delete propagates to a destination **through a connection** (not a raw engine `ConfiguredStream`), and
remove the now-false "not yet surfaced" caveats from the docs.

## Reference

- `crates/weir-engine/tests/wasm_postgres_engine.rs` — the T-0117 delete-propagation harness (drives a
  `ConfiguredStream`); mirror it via `App` + a real connection configured `cdc` / `upsert`.
- The connection modes from [[WEIR-T-0129]].
- `docs/guides/cdc-deletes.md` + `docs/reference/connection-config.md` — the caveats to remove.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `connection_modes_round_trip_and_validate` — a cdc/upsert connection **persists + reloads** its modes +
  business keys through `App`, and an incomplete upsert is **rejected**. (The derived CDC stream's live
  delete-landing is proven by the T-0117 harness; `work_spec`'s wiring by the weir-app unit test — see the
  design note below on why a single self-contained live pg→pg-through-a-connection test isn't expressible.)
- [x] Docs: `guides/cdc-deletes.md` rewritten as a real how-to (configure a CDC connection); the false caveats
  removed from it + `reference/connection-config.md`; mode fields + CLI flags added to the reference.
- [x] `mkdocs build --strict` clean; weir-app tests + clippy clean.

## Status Updates

### 2026-07-08 — done (`c087345`), closes I-0028

Modes are now real for operators + the docs tell the truth. **Scope note on the e2e:** a connection shares one
`config` between source + dest, so a **pg → pg** CDC connection points both at the same url/table — a loop that
can't be a single self-contained live test. So the proof is the **composition**: `connection_modes_round_trip_
and_validate` proves the App connection carries + reloads the modes and validates them; the weir-app unit test
proves `work_spec` derives `SyncMode::Cdc` / `WriteMode::Upsert{keys}`; and T-0117 proves that exact
`ConfiguredStream` propagates a delete (hard, tombstone, and REST) against live Postgres. Documented the
shared-config constraint in the how-to. **Complete — closes [[WEIR-I-0028]].**
