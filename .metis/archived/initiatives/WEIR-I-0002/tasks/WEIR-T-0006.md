---
id: end-to-end-round-trip-conformance
level: task
title: "End-to-end round-trip conformance test over both primitives"
short_code: "WEIR-T-0006"
created_at: 2026-06-17T21:25:21.162020+00:00
updated_at: 2026-06-18T13:00:34.880558+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# End-to-end round-trip conformance test over both primitives

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

The **gate that ratifies [[WEIR-A-0014]]**: an automated end-to-end test that runs the reference source → destination round-trip through the Sync Engine, with the source executed **both** as a dylib and as a WASM component, proving the contract is transport-neutral over both primitives.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] E2E test: reference REST source → minimal Engine → reference Arrow destination, asserting data + schema fidelity end-to-end.
- [ ] The same source connector passes the test run **as a dylib** and **as a WASM component** (identical results) — proves transport-neutrality ([[WEIR-A-0002]]/[[WEIR-A-0014]] NFR-CT-3).
- [ ] Incremental re-run resumes from committed checkpoint (no duplicates beyond idempotent upsert; [[WEIR-A-0011]]).
- [ ] Secret-handle redemption verified to occur on the agent, plaintext never in the control plane ([[WEIR-A-0013]]).
- [ ] This becomes the seed of the **connector acceptance-test harness** (NFR-CT-4).
- [ ] On green: move [[WEIR-A-0014]] to `decided`.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
This is the conformance harness seed reused by migration fidelity ([[WEIR-A-0020]]).

### Dependencies
All prior Slice-1 tasks ([[WEIR-T-0001]]–[[WEIR-T-0005]]).

## Status Updates **[REQUIRED]**
*To be added during implementation*
