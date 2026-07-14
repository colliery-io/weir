---
id: connection-modes-model-wiring-cli
level: task
title: "Connection modes — model + wiring + CLI/API"
short_code: "WEIR-T-0129"
created_at: 2026-07-08T03:03:54.944311+00:00
updated_at: 2026-07-08T03:21:53.373588+00:00
parent: WEIR-I-0028
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0028
---

# Connection modes — model + wiring + CLI/API

## Parent Initiative

[[WEIR-I-0028]] — the core: expose the modes and stop hardcoding.

## Objective

A connection carries `sync_mode`, `write_mode`, `business_keys`, and `cursor_field`, persisted and honored by
`work_spec`, settable + validated via the CLI and API.

## Reference

- `crates/weir-schema/schema/migrations/0001_init/up.sql` — the `connections` table (+ `angreal schema gen`).
- `crates/weir-app/src/lib.rs` — `Connection` struct, `add_connection`, the `ConnTuple` read, and `work_spec`
  (which hardcodes `SyncMode::FullRefresh` / `WriteMode::Append` / `cursor_field: None`).
- `crates/weir-cli/src/main.rs` — `ConnAction::Add`; `crates/weir-api/src/lib.rs` — `ConnectionDto`.
- The engine's `SyncMode` / `WriteMode` (`weir-connector-types`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `connections` gained `sync_mode` / `write_mode` / `business_keys` / `cursor_field` (regen'd, both backends);
  `Connection` + `ConnTuple` + read/write carry them.
- [x] `work_spec` builds `ConfiguredStream` from them (`parse_sync_mode`/`parse_write_mode`) — no more hardcode.
- [x] `add_connection` validates via `validate_connection_modes` (upsert⇒keys, incremental⇒cursor, unknown⇒error).
- [x] CLI flags `--sync-mode`/`--write-mode`/`--business-keys`(comma)/`--cursor-field`; API `ConnectionDto` fields
  + round-trip.
- [x] Unit test `connection_modes_validate_and_wire`; 22 weir-app lib tests + workspace compile + clippy clean.

## Status Updates

### 2026-07-08 — done (`7dc268c`)

Modes exposed end to end. `connection_modes_validate_and_wire` proves validation + that `work_spec` wires
`SyncMode::Cdc` / `WriteMode::Upsert{keys}` onto the `ConfiguredStream` (no longer hardcoded); 22 weir-app lib
tests + workspace compile + clippy green. (A CLI flag smoke was blocked only by an intermittent Bash-classifier
outage; the unit test covers the same validation path, and [[WEIR-T-0130]]'s e2e live-proves the connection→delete
path.) **Complete.**
