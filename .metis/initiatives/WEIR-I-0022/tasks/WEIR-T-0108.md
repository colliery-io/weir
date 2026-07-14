---
id: transform-mapping-depth-docs
level: task
title: "Transform/mapping depth + docs"
short_code: "WEIR-T-0108"
created_at: 2026-07-06T11:33:22.253090+00:00
updated_at: 2026-07-06T22:56:21.201730+00:00
parent: WEIR-I-0022
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0022
---

# Transform/mapping depth + docs

## Parent Initiative

[[WEIR-I-0022]] — closes the **transforms / mapping** partial.

## Objective

Turn the mapping stage from "exists" into "known and trusted": audit the supported transform primitives,
**depth-test** each, fill the obvious gaps, and **document the supported set** so authors know what they can rely on.

## Reference

- `crates/weir-engine/src/mapping.rs` — the mapping stage.
- `crates/weir-connector-types/src/types.rs` — `MappingSpec` and the mapping shape.
- `crates/weir-engine/src/lib.rs` — where mapping is applied in the record path.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Audit** — 6 primitives: `Select` / `Drop` / `Rename` / `Cast` (str/int/float/bool) / `Filter`
  (6 comparators) / `Compute` (const/field/concat/lower/upper). Written up in the docs.
- [x] **Depth tests** — 6 new edge-case tests: absent-field no-ops, null-passthrough vs uncastable dead-letter,
  scalar cross-casts, filter-on-absent drops, unmapped passthrough + op ordering, non-object passthrough (10/10).
- [x] **Gap filled** — `Cast` on `null` now passes through (SQL-nullable) instead of dead-lettering the record.
  Nested-path access noted as a by-design non-goal (not a low-cost fix — every op would need path traversal).
- [x] **Docs** — `docs/guides/field-mapping.md` (operators table, defined edge behaviour, worked example,
  non-goals), wired into the mkdocs nav.
- [x] Workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`fc537a8`)

Audited `mapping.rs` → 6 primitives. **Gap fix**: `Cast(null)` → passthrough (was: dead-letter the whole row).
Added 6 depth tests pinning edge-case behaviour (10/10 lib tests). New `docs/guides/field-mapping.md` reference
+ nav entry — operators, the *defined* edge behaviour (absent/null/uncastable/passthrough/ordering), a worked
example, and the by-design non-goals (nested paths, numeric compute, cross-record). clippy clean. **Complete —
closes the transform/mapping partial.**
