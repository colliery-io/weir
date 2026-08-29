---
id: connector-packaging-hygiene-drift
level: task
title: "Connector packaging hygiene — drift guard covers all guests, stage s3"
short_code: "WEIR-T-0172"
created_at: 2026-08-16T15:24:11.553176+00:00
updated_at: 2026-08-29T02:13:50.630785+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


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

- [x] The drift guard and the angreal GUESTS list cover every guest under `crates/connectors/*` and `wasm-fixtures/*` — ideally discovered by glob so a new guest can never silently escape; otherwise a test fails when an uncovered guest dir appears
- [x] `angreal connectors sync-contract` touches all guests; the drift test is green afterwards
- [x] s3 is staged by `stage-connectors.sh` and therefore present in the Docker image
- [x] `angreal test unit` and `angreal test manifests` green; installation docs' staged list updated in [[WEIR-T-0174]]

## Implementation Notes

Running `sync-contract` over the newly covered guests may produce real diffs in mssql/snowflake `weir_guest_types.rs` — if so, rebuild and rerun their (docker-gated) engine tests rather than assuming byte-identity was already true. Staging the resident fixture is optional (it is test-only); decide and note either way.

## Status Updates **[REQUIRED]**

**2026-08-28 — completed.** All list holes closed by making the lists glob-derived (impossible to drift):

- **Drift guard globs**: `contract_drift` in `crates/weir-codegen/src/lib.rs` rewritten to discover guests by directory scan over `../connectors` and `../../wasm-fixtures` (a guest = dir with `Cargo.toml` + `src/`); asserts each carries `src/weir_guest_types.rs` byte-identical to the canonical block, with a floor `checked >= 11` so a moved root can't silently zero the guard. Result: 11 guests checked (6 connectors incl. mssql/snowflake + 5 fixtures incl. resident) — 2/2 lib tests green.
- **angreal GUESTS globs**: `.angreal/task_connectors.py` GUESTS now `_guest_dirs()` (same glob over both roots); `angreal connectors sync-contract` reports all 11 guests, **no diffs produced** — mssql/snowflake copies were already byte-identical, so no rebuild/engine-test rerun was needed.
- **s3 staged**: `scripts/stage-connectors.sh` gains `stage crates/connectors/s3 weir_s3_wasm.wasm weir-s3-pkg http` — s3 now lands in every staged dir and the Docker image. `weir-wasm-testkit::connectors_dir()` also stages the production set (incl. s3), which WEIR-T-0166's resolution-validation tests rely on.
- **Resident fixture**: deliberately NOT staged — it is a test-only fixture; it is covered by the drift guard and sync-contract but does not belong in the shipped connectors dir.
- **Verification**: `angreal test unit` all 12 binaries green; `angreal test manifests` green (1 passed, 4 ignored live); `angreal check all` clean. Installation docs' staged list updated under [[WEIR-T-0174]].
