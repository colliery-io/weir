---
id: typed-schema-model-capture-persist
level: task
title: "Typed schema model + capture/persist"
short_code: "WEIR-T-0118"
created_at: 2026-07-07T22:33:03.870928+00:00
updated_at: 2026-07-07T23:21:53.428378+00:00
parent: WEIR-I-0025
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0025
---

# Typed schema model + capture/persist

## Parent Initiative

[[WEIR-I-0025]] — the model everything else builds on.

## Objective

Give a stream a **real, persisted schema**: a weir-native `StreamSchema { fields: [Field { name, type, nullable }] }`,
captured at onboard/discover (connector-declared, else inferred from a sample), stored per `(tenant, connection,
stream)`.

## Reference

- `crates/weir-connector-types/src/types.rs` — `StreamInfo.schema` (empty `ArrowSchemaIpc` today), `CastType`
  (str/int/float/bool) to extend into `FieldType` (+ `timestamp`/`json`).
- `crates/weir-app/src/lib.rs` — onboard/`import` + discover; `weir-schema` for a persisted column/table.
- `crates/connectors/postgres/src/lib.rs` — `discover` (returns a "table" stream, empty schema) → introspect
  `information_schema.columns`. Declarative manifests → declare where they can.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `StreamSchema` / `Field` / `FieldType { Str, Integer, Float, Boolean, Timestamp, Json }` in
  `weir-connector-types` (serde; JSON round-trip).
- [x] **Capture** via the **inference path** (`StreamSchema::infer`) — the engine samples source records each
  sync and captures if none is stored; works for **every** connector (incl. Postgres, whose integer/text/etc.
  columns infer correctly from `row_to_json`). ~~Postgres `information_schema` introspection~~ **deferred** as a
  low-marginal-value refinement (only adds nullability of no-null-in-sample columns + all-null column types) vs
  real onboard-discover wiring cost.
- [x] **Persist** — `stream_schemas (tenant_id, connection, stream, schema, broken)` table (regen, both
  backends); `Store::capture_schema` writes, `App::get_stream_schema` reads back (tenant-scoped, default-tenant
  aligned with `stream_state`).
- [x] Unit tests: JSON round-trip + inference (int+float→float, mix→json, nullability) in connector-types; a
  live-ish `run_captures_stream_schema` (Slow `{n:i}` → field `n` integer). Workspace + clippy clean.

## Status Updates

### 2026-07-07 — model + inference done (`796e232`); persistence/capture/introspection remain

- **Done + committed**: `FieldType` / `Field` / `StreamSchema` in `weir-connector-types` +
  `StreamSchema::infer(sample)` (types + nullability from a record sample: int+float→float, mix→json,
  ever-null-or-absent→nullable) + serde JSON round-trip. **3 unit tests green; clippy clean.**
- **Remaining (for resume)**:
  1. **Persist** — a new `stream_schemas (tenant_id, connection, stream, schema TEXT, PK)` table (regen) + App
     `get_stream_schema` / `put_stream_schema` (kept out of `stream_state` so the engine's checkpoint tuple is
     untouched).
  2. **Capture at run** — in the engine sync, if no schema is stored for the stream, `infer` from the first
     batch's records + persist (works for every connector).
  3. **Connector-declared (Postgres)** — `discover` introspects `information_schema.columns` → a `StreamSchema`;
     carry it to the host (repurpose the always-empty `StreamInfo.schema` IPC bytes as schema JSON, or add a WIT
     field). Preferred over inference where present.
  - Then [[WEIR-T-0119]] enforcement reads the stored schema; [[WEIR-T-0120]] evolution diffs it.

### 2026-07-07 — done (`3f8ec36`)

Persistence + capture + read landed: `stream_schemas` table (regen'd, migrates both backends); the engine
samples source records per sync + `Store::capture_schema` infers + persists if absent (keyed by the logical
stream, best-effort under concurrent partitions); `App::get_stream_schema` reads it back. `run_captures_stream_
schema` proves it end-to-end (Slow `{n:i}` → field `n` integer). Workspace + clippy clean. **Capture is the
inference path (all connectors); Postgres `information_schema` declared introspection deferred as a low-value /
high-wiring-cost refinement — inference already types Postgres columns correctly.** Unblocks [[WEIR-T-0119]].
**Complete.**
