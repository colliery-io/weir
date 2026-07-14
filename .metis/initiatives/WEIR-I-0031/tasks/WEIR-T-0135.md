---
id: canonical-contract-block-codegen
level: task
title: "Canonical contract block + codegen emit + drift guard"
short_code: "WEIR-T-0135"
created_at: 2026-07-09T00:08:37.825316+00:00
updated_at: 2026-07-09T00:22:56.975975+00:00
parent: WEIR-I-0031
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0031
---

# Canonical contract block + codegen emit + drift guard

## Parent Initiative

[[WEIR-I-0031]] — the first of two: establish the single canonical block + wire `weir-codegen` to it + the guard.

## Objective

Make one canonical guest-contract block the single source (with `Changes`/`ChangeRecord`/`ChangeOp` restored),
rewire `weir-codegen` to emit `mod weir_guest_types;` + a `src/weir_guest_types.rs` from it (deleting the stale
~125-line static block embedded in `wasm.rs`), and add a drift-guard test. Prove a generated guest still builds.

## Reference

- `crates/weir-codegen/src/wasm.rs` — the stale static block inside `lib_rs` (~:131–256; `RecordBatch { Rows, Arrow }`
  at :222, **missing `Changes`**); `generate()` (:18) emits Cargo.toml/build.rs/lib.rs.
- Live block shape (canonical target): `crates/connectors/rest/src/lib.rs` — `mod weir_guest_types` (:8), `Changes`
  (:114), `ChangeRecord` (:118), `ChangeOp`.
- `crates/weir-connector-types/src/types.rs` — host serde contract the WitType block mirrors (`ChangeOp` :253,
  `ChangeRecord` :263, `RecordBatch` :271).
- Mechanism (validated): `fidius-wit` (`generate.rs:101–120`) follows external `mod m;` → `src/m.rs`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] Canonical block is one file — `crates/weir-codegen/src/guest_contract.rs.in` (`include_str!` as
  `GUEST_CONTRACT`). It restores `Changes`/`ChangeRecord`/`ChangeOp` **and** the `Cast`/`Filter`/`Compute` mapping
  ops + `CastType`/`CompareOp`/`ComputeExpr` — the template had lost those too, not just `Changes`.
- [x] `generate()` emits `mod weir_guest_types;` + a `src/weir_guest_types.rs` = canonical; the ~125-line inline
  static block is **deleted** from `wasm.rs`.
- [x] `weir-codegen` drift-guard test: emitted `weir_guest_types.rs` == `GUEST_CONTRACT` + contains `Changes`, and
  `lib.rs` no longer inlines the block.
- [x] **Built for `wasm32-wasip2` end-to-end** — restructured the `rest` guest to the external mod, real rebuild
  (3.18s), `emit_wit` followed `mod weir_guest_types;` + regenerated the wit with `change-record` /
  `changes(list<change-record>)`, and all 16 `wasm_http_engine` host tests pass (interface hash unchanged).
- [x] `weir-codegen` builds + tests + clippy green.

## Implementation Notes

### Technical Approach
The canonical is the exact `mod weir_guest_types { … }` body (the 35 `WitType` types) as one string/const. The emitted
`src/weir_guest_types.rs` keeps `use super::*;` (so `WitType` etc. resolve from the guest `lib.rs`'s imports) — inline
vs external produces the same `mod_path`, so the interface hash is unchanged. `generate()` gains a
`(src/weir_guest_types.rs, canonical)` file and `lib_rs` swaps the inline block for `mod weir_guest_types;`. Do **not**
run `cargo fmt` (stale-toolchain drift, per [[WEIR-T-0133]]); hand-format.

### Dependencies
None. [[WEIR-T-0136]] applies the same canonical to the full-code connectors + fixtures and extends the guard.

### Risk Considerations
The external-mod file must compile as a module (imports resolve) and `emit_wit` must find the types through it —
both proven by the `wasm32-wasip2` build in the AC. If `use super::*;` doesn't resolve standalone, give the file its
own `use fidius_macro::WitType;` (+ serde) imports.

## Status Updates

### 2026-07-09 — done

The seam was even more broken than the review found: the `wasm.rs` template had lost not only `Changes` but the
`Cast`/`Filter`/`Compute` mapping ops + their enums — so *every* manifest connector generated since would have been
missing half the mapping surface. The canonical (`guest_contract.rs.in`, taken from the hand-patched `rest` block)
restores all of it.

Mechanism proven with a live wasm build rather than by inference: `rest` restructured to `mod weir_guest_types;` +
an external `src/weir_guest_types.rs` (byte-identical to the canonical), rebuilt for `wasm32-wasip2` in 3.18s —
`emit_wit` followed the external mod, regenerated the wit with the change types, and all 16 `wasm_http_engine`
host tests pass (contract unchanged → interface hash unchanged). So **`rest` is already migrated** (1 of the 8
guests); [[WEIR-T-0136]] does the other 7 (postgres/rest-dest/s3 + the 4 fixtures) + the `sync-contract` generator
+ the drift guard extended to every checked-in copy. No `cargo fmt` (stale-toolchain drift).
