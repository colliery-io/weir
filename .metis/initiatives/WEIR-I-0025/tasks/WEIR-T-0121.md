---
id: surface-schema-drift-in-the-ui
level: task
title: "Surface schema + drift in the UI"
short_code: "WEIR-T-0121"
created_at: 2026-07-07T22:33:07.750384+00:00
updated_at: 2026-07-07T23:57:03.538712+00:00
parent: WEIR-I-0025
blocked_by: [WEIR-T-0118, WEIR-T-0120]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0025
---

# Surface schema + drift in the UI

## Parent Initiative

[[WEIR-I-0025]] — the operator-facing part. Closes the initiative.

## Objective

Make the schema + drift visible: a per-connection **schema view** (fields + types + nullability) and a **drift /
needs-attention** signal so an operator sees a breaking change without reading logs.

## Reference

- `weir-ui/src/main.rs` — the run-detail modal + the Health dashboard ([[WEIR-T-0111]]/[[WEIR-T-0112]]); the
  `apath` tenant-scoped fetch wrappers.
- New endpoint(s): `GET .../schema` for a connection's stored `StreamSchema` (from [[WEIR-T-0118]]); the
  breaking-drift state from [[WEIR-T-0120]] (a run error / connection flag).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Schema view** in the run-detail modal: each field's `name · type · nullability`, via
  `GET /connections/{name}/schema` (`SchemaView`).
- [x] **Drift signal**: a red banner with the breaking reason + an "Accept new schema" button renders when the
  `broken` flag is set (data-driven); a healthy/additive schema just lists fields, no alarm.
- [x] Tenant-scoped via the existing `apath`/`areq` wrappers; empty state ("no schema captured yet") handled.
- [x] API test (run captures → `/schema` returns `n:integer`, `broken` null) + Playwright e2e (detail modal
  shows the schema section). Full weir-api suite (17) + all 10 e2e green; UI wasm compiles; clippy clean.

## Status Updates

### 2026-07-07 — done (`006fd7e`), closes I-0025

`GET /connections/{name}/schema` serves a `SchemaView` (typed fields + `broken` drift flag); `POST .../schema/
accept` is the operator escape hatch (→ `App::accept_schema`); both added to the default-deny authz table.
The run-detail modal lists each field's name/type/nullability and shows a red drift banner + Accept button when
flagged. API test + a Playwright e2e both green (17 API tests, 10 e2e); UI wasm compiles; clippy clean.
**Complete — closes [[WEIR-I-0025]].** Note: the drift surfaces in the connection detail (not a separate Health
"needs attention" row) — a reasonable v1; a Health-grid drift badge could be a small follow-up.
