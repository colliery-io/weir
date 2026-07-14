---
id: in-flight-transforms-addfields
level: task
title: "In-flight transforms — AddFields/RemoveFields/record_filter → MappingSpec on the connection"
short_code: "WEIR-T-0071"
created_at: 2026-07-04T02:28:19.006633+00:00
updated_at: 2026-07-04T02:47:48.735909+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# In-flight transforms — AddFields/RemoveFields/record_filter → MappingSpec on the connection

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]]. Tracked in [[WEIR-S-0016]] (record-select rows). **Split out of [[WEIR-T-0068]]** — the
request-options half (POST/body/headers) landed there; transforms need a distinct plumb.

## Objective **[REQUIRED]**

Translate Airbyte record transforms — `AddFields` / `RemoveFields` / key transforms + `record_filter` —
onto weir's **existing in-flight mapping stage** ([[WEIR-T-0052]]: `MappingOp` Compute/Drop/Rename/Filter),
so a manifest's per-stream transforms shape the emitted records at run time (no runtime/connector change).

## Why it's its own task (the wire-up)

The mapping stage exists (engine-side) and `MappingOp` covers the ops. The gap is **plumbing the manifest's
transforms to the `ConfiguredStream.mapping`**:
- Today `weir-app::work_spec(c: &Connection)` builds `ConfiguredStream { …, mapping: MappingSpec::default() }`
  and has **no manifest access** — it only sees the `Connection` (name/source/dest/stream/config).
- So a manifest→mapping path is needed: either (a) `work_spec` (or its caller) looks up the source
  manifest by slug (via the catalog / `App`) and builds the `MappingSpec` from the stream's transforms, or
  (b) the transforms are carried on the manifest → onto the connection at creation → into `work_spec`.
- This touches the connection/run architecture, which is why it's separated from the self-contained
  request-options work.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **Manifest carries transforms** (per stream): a small `Vec<Transform>` or reuse of a mapping shape
  that maps onto `MappingOp`.
- [ ] **Importer** maps Airbyte `transformations` (`AddFields` → `Compute`, `RemoveFields` → `Drop`, key
  rename → `Rename`) + `record_filter` → `Filter`. Unsupported sub-forms reported (not dropped). Unit tests.
- [ ] **Wire-up**: `work_spec` / the connection→run path builds `ConfiguredStream.mapping` from the source
  manifest's stream transforms (decide (a) or (b) above).
- [ ] **Proof**: an engine test where AddFields/RemoveFields/record_filter change the emitted records
  (the mapping stage already has unit coverage; this proves the manifest→mapping wiring).
- [ ] **Ledger**: [[WEIR-S-0016]] `record_filter` + `AddFields/RemoveFields` rows → ✅; `analyze()` updated.
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes

- `MappingOp` / `MappingSpec` live in `weir-connector-types/src/types.rs`; the engine applies them (see
  [[WEIR-T-0052]]). Keep within the dbt boundary ([[WEIR-A-0026]]) — no joins/aggregates/UDFs.
- Lower value than the rest of the parity arc (no corpus connector *requires* transforms for basic
  function), hence deferred — but it completes the declarative-transform story.

## Status Updates **[REQUIRED]**

### 2026-07-04 — plan decided: carry MappingSpec on the baked config (`__mapping`)

**Key constraint (confirmed by reading the code):** `add_connection` → `resolve_manifest_source` rewrites
the source to `"rest"` and bakes the manifest→config; the **manifest slug is then lost** from the stored
connection. `work_spec(&Connection)` (the only place a `ConfiguredStream` is built, weir-app:963) has no
manifest access. So the mapping must be derived **at connection-create time** (manifest in hand) and
**carried on the baked config** — option (b).

**Approach (minimal — no `work_spec` sig change, no DB migration):**
1. `weir-manifest`: `Stream` gains `mapping: MappingSpec` (dep `weir-connector-types`; acyclic, fidius-free
   without the `wit` feature). Round-trips the stored weir-manifest YAML.
2. `weir-importer` (dep `weir-connector-types`): map Airbyte `transformations` + `record_filter` →
   `MappingSpec` — `RemoveFields`→`Drop`, `AddFields`→`Compute` (`Const` literal / `Field` `{{ record['x'] }}`),
   `record_filter`→`Filter` (parse simple `record['f'] OP val`). Complex forms reported, not dropped.
3. `weir-app::manifest_stream_to_config`: if the stream's `MappingSpec` is non-empty, embed it as
   `config["__mapping"]` (JSON).
4. `weir-app::work_spec`: lift `__mapping` off the config → `ConfiguredStream.mapping`; strip it from the
   guest `config`. The engine's existing mapping stage ([[WEIR-T-0052]]) applies it.
5. Tests: importer unit (transforms→ops) + an engine test (records actually reshaped) + ledger flip.

**Scope:** clean/unambiguous forms (RemoveFields, literal/field AddFields, simple record_filter); jinja
compute/condition grammar beyond that is reported.

### 2026-07-04 — implemented + tested; **task complete**

The `__mapping` plan landed exactly as designed, plus `analyze()` reporting.
- `weir-manifest`: `Stream.mapping: MappingSpec` (dep `weir-connector-types`; round-trips YAML).
- `weir-importer`: `map_transforms` lowers `RemoveFields`→`Drop`, `AddFields`→`Compute`
  (`Const`/`Field` via `compute_expr`), `record_filter`→`Filter` (`filter_op`). Parsers `record_field` /
  `compute_expr` / `filter_op` reject compound/expr grammar (require a lone `record['id']` + a single
  operator, RHS with no second field ref) so junk never becomes a wrong op. `analyze()` now reports the
  rejected forms (complex AddFields values, unknown transforms, compound conditions) — never silently
  dropped ([[WEIR-A-0020]]).
- `weir-app`: `manifest_stream_to_config` embeds the spec as `__mapping`; `work_spec` → `extract_mapping`
  lifts it onto `ConfiguredStream.mapping` and **strips it from the guest config** (sandbox never sees it).
- Engine already applies `stream.mapping` in the sync path (weir-app:425 → the [[WEIR-T-0052]] stage).

**Tests (all green):**
- `imports_transforms_to_mapping` (importer 15/15): AddFields(literal+field) / RemoveFields / record_filter
  → the exact `MappingOp` vec.
- `analyze_reports_complex_transforms`: compound condition + `~`-concat AddFields → reported.
- `work_spec_lifts_mapping_off_baked_config` (weir-app 8/8): `__mapping` lifted + config stripped.
- `engine_applies_transform_mapping_over_wasm_source` (wire): 3 records → `Filter(keep==yes)` → 2 written,
  end-to-end over real `wasi:http`.

**Ledger:** [[WEIR-S-0016]] `record_filter` + `AddFields/RemoveFields` rows → ✅; complex grammar split to a
reported ❌ row.

**AC status:** all met (manifest carries transforms ✅, importer maps + reports ✅, wire-up ✅, engine proof
✅, ledger + analyze ✅). `Rename` (Airbyte key-transform) not emitted — no common Airbyte construct maps to
it cleanly; folded into the reported "complex grammar" row.
