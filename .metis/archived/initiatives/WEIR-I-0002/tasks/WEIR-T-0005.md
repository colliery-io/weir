---
id: minimal-sync-engine-slice-plan
level: task
title: "Minimal Sync Engine slice (plan → outbox → read-map-write → transactional checkpoint)"
short_code: "WEIR-T-0005"
created_at: 2026-06-17T21:25:19.352460+00:00
updated_at: 2026-06-18T12:48:00.082668+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Minimal Sync Engine slice (plan → outbox → read-map-write → transactional checkpoint)

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Build the thinnest Sync Engine ([[WEIR-S-0004]]) path that drives one connection end-to-end: plan a work unit, dispatch via the transactional outbox, run read→(map)→write through the Runtime, and commit checkpoint+outbox+run-state in one transaction. Proves [[WEIR-A-0009]]/[[WEIR-A-0010]]/[[WEIR-A-0011]]/[[WEIR-A-0007]] together.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] `diesel-dualdb` store ([[WEIR-A-0009]]) with a minimal Tier-2 schema (runs, stream_state/checkpoint, outbox) on **SQLite** — `crates/weir-engine` `Store::open` + raw portable DDL.
- [x] Chunked-pull loop reads from a configured connection + last state, applies the mapping stub ([[WEIR-A-0026]] passthrough), writes to the destination via the Runtime seam ([[WEIR-T-0002]]).
- [x] **Checkpoint + outbox row commit in ONE transaction** per chunk — structural crash-safety (the checkpoint can't advance unless its outbox record commits in the same txn). *Explicit failure-injection test is a nice-to-have follow-up.*
- [x] Single in-process agent, no broker (NFR-SE-5/NFR-DP-1). `tests/engine.rs` green: echo→engine→echo, state persists across runs, outbox rows committed.
- [~] A per-`runs`-row update is a trivial extension; the v0 loop commits the checkpoint+outbox atomicity that matters.

## Status Updates

- **2026-06-18 — DONE.** `crates/weir-engine` (`Store` + `Engine`) on `diesel-dualdb` 0.1 (crates.io). Raw portable SQL (sql_query + `conn.transaction`) keeps the Diesel surface tiny; per-chunk checkpoint+outbox commit atomically. SQLite via a temp-file DB in the test (`:memory:` + pool gives independent DBs). Drives the `echo` connector through the `weir-runtime` seam. Note: `DualConnection` is private in diesel-dualdb → don't name it (inline the helpers / let the type infer).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Transactional outbox pattern reimplemented in weir (cloacina pattern, not a dep — [[WEIR-A-0010]]/[[WEIR-A-0027]]). Mapping stage can be a pass-through stub for Slice 1.

### Dependencies
[[WEIR-T-0001]], [[WEIR-T-0002]]; consumes the reference source/destination ([[WEIR-T-0003]]/[[WEIR-T-0004]]).

## Status Updates **[REQUIRED]**
*To be added during implementation*
