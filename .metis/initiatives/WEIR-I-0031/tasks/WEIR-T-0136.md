---
id: restructure-connectors-fixtures
level: task
title: "Restructure connectors + fixtures onto the canonical block"
short_code: "WEIR-T-0136"
created_at: 2026-07-09T00:08:39.250144+00:00
updated_at: 2026-07-09T02:53:34.565053+00:00
parent: WEIR-I-0031
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0031
---

# Restructure connectors + fixtures onto the canonical block

## Parent Initiative

[[WEIR-I-0031]] — the second of two: apply the canonical block to every full-code connector + fixture, and extend
the guard. Closes the initiative.

## Objective

Restructure the four full-code connectors (`postgres`/`rest`/`rest-dest`/`s3`) and the four wasm-fixtures
(`echo`/`slow`/`faulty`/`arrow-sink`) to `mod weir_guest_types;` + a generated `src/weir_guest_types.rs` from the
[[WEIR-T-0135]] canonical; delete the inline blocks; add a `sync-contract` generator + extend the drift guard to all
eight guests.

## Reference

- The canonical block + emit + guard from [[WEIR-T-0135]].
- The eight guests: `crates/connectors/{postgres,rest,rest-dest,s3}/src/lib.rs`,
  `wasm-fixtures/{echo,slow,faulty,arrow-sink}/src/lib.rs` — each with an inline `mod weir_guest_types { … }`.
- `.angreal/task_connectors.py` (host the `sync-contract` task); `angreal test connectors` builds the wasm guests;
  `crates/weir-engine/tests/wasm_*_engine.rs` load the built guests (the interface-hash check is the real guard).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] All eight guests declare `mod weir_guest_types;` + carry a `src/weir_guest_types.rs` byte-identical to the
  canonical; inline blocks deleted. `echo` was the outlier (top-level types, not a mod) — converted to the mod.
- [x] `angreal connectors sync-contract` writes the canonical into every guest + collapses inline blocks;
  idempotent. `connectors new` also fixed to source the contract from the canonical (the line-position copy broke
  once `rest` lost its inline block) — verified it scaffolds a wasm-building connector.
- [x] Drift guard extended: `weir-codegen`'s `contract_drift` test asserts all eight checked-in copies ==
  `GUEST_CONTRACT`.
- [x] All eight build for `wasm32-wasip2` (4 fixtures via `angreal test connectors`, 4 connectors direct); the
  `wasm_*_engine` + fixture-load tests pass (interface hash uniform); `weir-codegen` tests + workspace clippy
  `-D warnings` green.

## Implementation Notes

### Technical Approach
Per guest: delete the inline `mod weir_guest_types { … }`, insert `mod weir_guest_types;`, write
`src/weir_guest_types.rs` = canonical. `emit_wit` follows the external mod (validated in [[WEIR-T-0135]]). Since every
guest now carries the **identical full** contract, their interface hashes are uniform and match the host — which is
the point. No `cargo fmt` (stale-toolchain drift); hand-format.

### Dependencies
[[WEIR-T-0135]] — provides the canonical + the generator + the guard to extend.

### Risk Considerations
A fixture whose inline block was a *subset* of the contract will now carry the full canonical, changing its emitted
`.wit` interface. That's intended (all guests speak the one contract), and the `wasm_*_engine` load tests — which
verify the interface-hash against the host — are the guard: if a guest's contract genuinely diverged, its load test
fails. Watch for a fixture that deliberately omitted `write`/`read` types; if one truly needs a narrower contract,
flag it rather than forcing the canonical.

## Status Updates

### 2026-07-09 — done, closes I-0031

`angreal connectors sync-contract` now regenerates all eight guests from the one canonical block; the seven that
weren't `rest` (done in [[WEIR-T-0135]]) migrated cleanly, except **`echo`**, which uniquely defined its `WitType`
types at top level (not in a `mod`) — converted it to `mod weir_guest_types;` (its wit reordered to canonical order,
types unchanged, still loads). Fixed `connectors new`, whose line-position block-copy broke once `rest` lost its
inline block. Added `contract_drift` (weir-codegen) asserting every checked-in copy == `GUEST_CONTRACT`.

Verified: all eight build for `wasm32-wasip2`; the fixture-load + `rest` engine tests pass (uniform interface hash);
weir-codegen tests + workspace clippy `-D warnings` green. The pg/s3 engine tests stay `#[ignore]` (need
Postgres/MinIO) but the guests compile. **Closes [[WEIR-I-0031]]** — the contract has one origin, generated into
every guest, drift-guarded, and the stale template (which had lost `Changes` *and* the Cast/Filter/Compute mapping
ops) is gone.
