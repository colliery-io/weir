---
id: typed-schemas-schema-evolution
level: initiative
title: "Typed schemas + schema evolution"
short_code: "WEIR-I-0025"
created_at: 2026-07-07T04:00:26.982611+00:00
updated_at: 2026-07-07T23:57:06.094829+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: typed-schemas-schema-evolution
---

# Typed schemas + schema evolution

## Context

The biggest **data-model** depth gap. Records flow as JSON-string rows and `ArrowSchemaIpc` in `StreamInfo` is
largely empty — there's no enforced typing, no schema captured at discover, and no handling when a source's shape
changes. Everything works untyped today, which is fine until a field is added/removed/retyped and a destination
silently breaks. This initiative gives streams a **real schema** and defines **evolution** behaviour.

## Goals

- **Capture a schema at discover** — each stream carries a typed field set (name, type, nullability), persisted
  in the catalog, not an empty IPC blob.
- **Enforce / coerce on the record path** — validate records against the stream schema (leaning on the existing
  mapping `Cast` machinery, [[WEIR-T-0108]]); mismatches dead-letter with a clear reason rather than corrupt a
  destination.
- **Evolution policy** — define + implement what happens on drift: additive (new nullable field) flows through;
  breaking changes (type change, dropped required field) are detected and surfaced (block / dead-letter /
  operator ack), not silently applied.

## Non-Goals (initially)

- A standalone external schema registry (start with the catalog as the registry).
- Full Avro/Protobuf schema import (JSON-shaped types first; formats can map onto it).

### Design decisions (2026-07-07, approved)

- **Type model**: **weir-native typed fields** — `StreamSchema { fields: Vec<Field { name, FieldType, nullable }> }`,
  `FieldType` extending `CastType` (str/int/float/bool) with `timestamp`/`json`. JSON-shaped, persists as JSON,
  reuses the mapping `Cast` machinery for coercion. Not Arrow-IPC.
- **Schema source**: **connector-declared + inference fallback** — Postgres introspects `information_schema` at
  discover; declarative manifests declare where they can; else weir infers from a record sample. Every stream
  gets a schema.
- **Enforcement**: **coerce, else dead-letter** — on the engine record path, coerce each field to its type via
  `Cast`; an uncoercible value or a missing *required* (non-nullable) field dead-letters that record with a
  reason; the batch continues; extra fields pass through.
- **Evolution**: **auto-accept additive, block/flag breaking** — on re-discover, diff vs the stored schema; a new
  nullable field flows through + updates the stored schema; a type change / dropped-required is detected +
  surfaced (the run flags/errors), not silently applied.
- Extends [[WEIR-A-0026]] (in-flight record path) + [[WEIR-A-0014]] (record encoding); no new ADR unless the
  model warrants one during T-a.

## Proposed decomposition (for sign-off)

- **T-a — Typed schema model + capture/persist:** `StreamSchema`/`Field`/`FieldType` in `weir-connector-types`;
  captured at onboard/discover (Postgres `information_schema` introspection; manifest-declared; **inference
  fallback** from a record sample); persisted per `(tenant, connection, stream)`.
- **T-b — Record-path enforcement:** an engine stage that coerces each record to its schema (via `Cast`) +
  dead-letters uncoercible / missing-required; composes with mapping; permissive on extra fields.
- **T-c — Schema evolution:** on re-discover, diff stored vs new → **auto-accept additive** (update stored),
  **detect + block/flag breaking** (type change / dropped required) as a run-level error surfaced to the operator.
- **T-d — Surface schema + drift (UI):** a per-connection **schema view** + a **drift / needs-attention** signal
  in the health view ([[WEIR-I-0024]]).

## Exit Criteria (draft)

- [ ] Streams carry a typed, persisted schema captured at discover.
- [ ] Records are validated/coerced against it; violations dead-letter with reasons.
- [ ] A defined + tested evolution policy for additive vs breaking drift.
