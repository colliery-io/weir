---
id: connector-packaging-hygiene-drift
level: task
title: "Connector packaging hygiene — drift guard covers all guests, stage s3"
short_code: "WEIR-T-0172"
created_at: 2026-08-16T15:24:11.553176+00:00
updated_at: 2026-08-16T15:24:11.553176+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Connector packaging hygiene — drift guard covers all guests, stage s3

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

Hand-maintained lists have drifted from the guest population: the contract drift guard and the angreal GUESTS list omit mssql + snowflake (both shipped connectors) and the resident fixture, so their `weir_guest_types.rs` copies can silently diverge from the canonical contract block; and `stage-connectors.sh` never stages s3, so no staged connectors dir — including the Docker image — contains it. Close the list holes, preferably by making the lists impossible to drift.

## Evidence (2026-08-16 alpha review)

- `crates/weir-codegen/src/lib.rs:155-177` — contract_drift hand-list missing mssql, snowflake, resident.
- `.angreal/task_connectors.py` GUESTS — same omissions; `angreal connectors sync-contract` silently skips two shipped guests.
- `scripts/stage-connectors.sh` — stages rest, rest-dest, snowflake, postgres, mssql + fixtures; no s3, no resident.
- Note only (fix belongs elsewhere): s3's ListObjectsV2 has no ContinuationToken loop, so buckets >1000 objects truncate — do not scope that in here.

## Acceptance Criteria **[REQUIRED]**

- [ ] The drift guard and the angreal GUESTS list cover every guest under `crates/connectors/*` and `wasm-fixtures/*` — ideally discovered by glob so a new guest can never silently escape; otherwise a test fails when an uncovered guest dir appears
- [ ] `angreal connectors sync-contract` touches all guests; the drift test is green afterwards
- [ ] s3 is staged by `stage-connectors.sh` and therefore present in the Docker image
- [ ] `angreal test unit` and `angreal test manifests` green; installation docs' staged list updated in [[WEIR-T-0174]]

## Implementation Notes

Running `sync-contract` over the newly covered guests may produce real diffs in mssql/snowflake `weir_guest_types.rs` — if so, rebuild and rerun their (docker-gated) engine tests rather than assuming byte-identity was already true. Staging the resident fixture is optional (it is test-only); decide and note either way.

## Status Updates **[REQUIRED]**

*To be added during implementation*
