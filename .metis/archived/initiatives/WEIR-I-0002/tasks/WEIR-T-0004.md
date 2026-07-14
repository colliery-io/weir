---
id: reference-native-arrow-destination
level: task
title: "Reference native Arrow destination (dylib)"
short_code: "WEIR-T-0004"
created_at: 2026-06-17T21:25:17.132565+00:00
updated_at: 2026-06-18T12:54:37.661579+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Reference native Arrow destination (dylib)

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Author one reference native (Rust) destination connector as a signed dylib that consumes the record stream and writes it, exercising the bulk Arrow path and `WriteMode`. Proves native-first-class ([[WEIR-A-0016]]) and the destination side of the contract.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] Native dylib destination implements `spec`/`check`/`write` against the contract.
- [ ] Consumes `Arrow(ipc_bytes)` record batches; supports `WriteMode::Append` and `WriteMode::Upsert{business_keys}` (the reverse-ETL flavor).
- [ ] Emits `Checkpoint`/`Trace` messages back to the Engine.
- [ ] Target is a trivial sink (e.g., local Parquet/duckdb or files) sufficient to verify round-trip — not a production warehouse.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Native dylib (trusted path). Idempotent writes to satisfy at-least-once ([[WEIR-A-0011]]).

### Dependencies
[[WEIR-T-0001]] (contract crate).

## Status Updates **[REQUIRED]**
*To be added during implementation*
