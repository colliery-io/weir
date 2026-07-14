---
id: reference-cli-api-contract-config
level: task
title: "Reference (CLI / API / contract / config)"
short_code: "WEIR-T-0127"
created_at: 2026-07-08T02:10:52.876423+00:00
updated_at: 2026-07-08T02:52:10.893506+00:00
parent: WEIR-I-0027
blocked_by: [WEIR-T-0125]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0027
---

# Reference (CLI / API / contract / config)

## Parent Initiative

[[WEIR-I-0027]] — dry, accurate, information-oriented.

## Objective

The lookup-oriented reference: what commands, endpoints, contract types, and config keys exist — precise, verified,
no narrative.

## Reference

- `crates/weir-cli/src/main.rs` — the `weir` subcommands (`init`, `connection`, `auth`, `serve`, `api`, …) +
  their flags (source of truth for the CLI ref).
- `crates/weir-api/src/lib.rs` — the route table (the endpoint list); `docs/api/index.md` (the generated ref).
- The connector contract: `RecordBatch` / `ChangeOp` / `ChangeRecord`, `SyncMode`, `WriteMode`, `FieldType`
  (weir-connector-types) + the WIT trait; mapping ops (from `guides/field-mapping.md`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **CLI reference** (`reference/cli.md`) — every `weir` command + flags, grouped (store/connections · daemons
  · auth · misc), from the clap defs.
- [x] **HTTP API** (`api/index.md` rewritten) — bearer auth, tenancy scoping, endpoint groups over the real route
  table; frames the generated ref.
- [x] **Connector contract** (`reference/connector-contract.md`) — the WIT trait + `RecordBatch`/`ChangeOp`/
  `SyncMode`/`WriteMode`/`FieldType`, verified against `weir-connector-types`.
- [x] **Connection config** (`reference/connection-config.md`) — fields, mapping, write modes + `on_delete`/
  tombstone, typed schemas; with the honest per-connection-mode caveat.
- [x] `mkdocs build --strict` clean.

## Status Updates

### 2026-07-08 — done (`5c85dca`)

Four reference pages + the API orientation, all verified against current code (route table, clap defs, the
contract enums). Cross-anchors (`#write-modes`, `#typed-schemas`) resolve; `--strict` clean. **Complete.**
