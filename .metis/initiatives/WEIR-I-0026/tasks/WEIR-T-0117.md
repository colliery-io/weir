---
id: end-to-end-delete-propagation
level: task
title: "End-to-end delete-propagation fidelity"
short_code: "WEIR-T-0117"
created_at: 2026-07-07T11:34:16.141451+00:00
updated_at: 2026-07-07T20:25:33.121914+00:00
parent: WEIR-I-0026
blocked_by: [WEIR-T-0114, WEIR-T-0115, WEIR-T-0116]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0026
---

# End-to-end delete-propagation fidelity

## Parent Initiative

[[WEIR-I-0026]] — the proving gate. Closes the initiative.

## Objective

Prove delete propagation **end-to-end, live**: a row deleted at the source is removed (hard) or tombstoned at the
destination — exactly once, in order — for both the postgres and rest destinations.

## Reference

- `crates/weir-engine/tests/wasm_postgres_engine.rs` — the CDC fidelity harness ([[WEIR-T-0107]]) to extend;
  live Postgres via `WEIR_PG_HOST_PORT`/`WEIR_TEST_PG_URL` (weir's own compose stack, coexists with cloacina).
- The change contract ([[WEIR-T-0113]]) + source ([[WEIR-T-0114]]) + dests ([[WEIR-T-0115]]/[[WEIR-T-0116]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **pg → pg hard delete** — `cdc_hard_delete_propagates_pg_to_pg`: dest row appears → updates → **gone**;
  replay changes nothing.
- [x] **pg → pg tombstone** — `cdc_tombstone_delete_propagates_pg_to_pg`: `on_delete=tombstone` → row remains,
  `_deleted_at` stamped.
- [x] **CDC → rest-dest** — `cdc_delete_propagates_to_rest_dest`: a delete → `DELETE /items/1` captured by a stub.
- [x] Ordering: I→U→D ends deleted (the hard-delete test).
- [x] `#[ignore]` integration lane; **11/11 postgres suite on a fresh DB**; clippy clean.

## Status Updates

### 2026-07-07 — done (`e15d656`), closes I-0026

Three delete-propagation tests, **proven live** (whole postgres suite 11/11 on a fresh integration DB): pg→pg
hard delete (appears/updates/gone + replay-noop), pg→pg tombstone (row kept + `_deleted_at`), and CDC→rest-dest
(`DELETE /items/1` to a stub HTTP endpoint). **Required fix**: the CDC source now **filters `test_decoding` to
its configured table** (the logical slot is whole-DB — else a Postgres dest's own writes feed back);
`parse_test_decoding` returns the table, `cdc_read` filters. Ordering (I→U→D ends deleted) proven; clippy clean.
**Complete — closes [[WEIR-I-0026]].**
