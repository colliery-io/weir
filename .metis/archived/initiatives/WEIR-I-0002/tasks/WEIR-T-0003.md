---
id: reference-declarative-rest-source
level: task
title: "Reference declarative REST source (manifest v0 → codegen)"
short_code: "WEIR-T-0003"
created_at: 2026-06-17T21:25:15.467264+00:00
updated_at: 2026-06-18T13:29:49.521778+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Reference declarative REST source (manifest v0 → codegen)

## Parent Initiative
[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Implement the manifest v0 schema ([[WEIR-S-0006]]) and a codegen pass that lowers a manifest into a `fidius` `Connector`, then author one reference declarative REST source from a manifest. Proves the declarative-first/codegen path ([[WEIR-A-0014]] §2) and the AI/migration authoring surface.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] Manifest v0 schema implemented (`weir-manifest`: spec/auth/streams/Arrow schema/incremental/pagination + partitioning via `PartitionScheme`) with YAML parse + validation.
- [x] Codegen (`weir-codegen`) produces a compilable `fidius` connector implementing `spec`/`check`/`discover`/`read` — the generated `crates/weir-connector-rest` builds as a workspace member.
- [x] Reference source `manifests/rest.yaml` → emits `Rows` projected to its schema, with **incremental cursor + pagination**. `weir-engine/tests/rest.rs`: paginates a mock API (2+1 records) through the engine to the Arrow sink, cursor advances to the max.
- [x] Generated connector builds to **both** a dylib (workspace) **and** a WASM component (`wasm-fixtures/rest`, `weir-codegen/tests/wasm_build.rs`). Codegen emits the **self-contained** guest (own interface + WitType types + `emit_wit`) — this is exactly why [[WEIR-A-0014]] chose codegen.

## Status Updates

- **2026-06-18 — DONE (all 4 criteria).** Built `weir-manifest` (schema), `weir-codegen` (lib + `weir-codegen <manifest> <dylib|wasm> <outdir>` CLI). Dylib target does real paginated HTTP (`ureq`), base_url overridable via config; WASM target is the self-contained guest (no registry macro — that's cdylib-only; macros from `fidius_macro`; needs `[workspace]` to build standalone; HTTP-in-sandbox needs a `wasi:http` grant, so its `read` is a stub while discover works → still proves codegen→component). Generated crates are committed + re-runnable. Note: committed dylib crate is `cargo fmt`'d so it differs from raw codegen output (codegen could run rustfmt — refinement); the codegen test asserts shape, not byte-equality.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Codegen, not a runtime interpreter ([[WEIR-A-0014]]). Manifest `schema:` declares Arrow logical types.

### Dependencies
[[WEIR-T-0001]] (contract crate). Aligns with migration importer target ([[WEIR-A-0020]]).

## Status Updates **[REQUIRED]**
*To be added during implementation*
