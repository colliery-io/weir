---
id: s1-in-flight-mapping-stage-cast
level: task
title: "S1: In-flight mapping stage (cast/filter/compute + expression AST)"
short_code: "WEIR-T-0052"
created_at: 2026-06-24T09:42:25.148668+00:00
updated_at: 2026-06-24T10:04:04.672542+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S1: In-flight mapping stage (cast/filter/compute + expression AST)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]]

## Objective **[REQUIRED]**

S1 of [[WEIR-I-0008]] — the **in-flight mapping stage** ([[WEIR-A-0026]] v0), engine-owned, applied between
read and write. Extend `MappingOp` beyond Select/Drop/Rename with **Cast / Filter / Compute** backed by a
**bounded expression AST + evaluator**; apply `MappingSpec` in `weir-engine` over row-JSON (+ Arrow
projection/filter); enforce the **dbt boundary** (light in-flight T only — heavy transforms stay post-load,
[[WEIR-A-0026]]). Independent of the migration machinery; **also unblocks reverse-ETL ([[WEIR-I-0007]])**.
Airbyte `AddFields`/`RemoveFields`/transforms (S6) map onto this.

## Backlog Item Details **[CONDITIONAL: Backlog Item]**

{Delete this section when task is assigned to an initiative}

### Type
- [ ] Bug - Production issue that needs fixing
- [ ] Feature - New functionality or enhancement
- [ ] Tech Debt - Code improvement or refactoring
- [ ] Chore - Maintenance or setup work

### Priority
- [ ] P0 - Critical (blocks users/revenue)
- [ ] P1 - High (important for user experience)
- [ ] P2 - Medium (nice to have)
- [ ] P3 - Low (when time permits)

### Impact Assessment **[CONDITIONAL: Bug]**
- **Affected Users**: {Number/percentage of users affected}
- **Reproduction Steps**:
  1. {Step 1}
  2. {Step 2}
  3. {Step 3}
- **Expected vs Actual**: {What should happen vs what happens}

### Business Justification **[CONDITIONAL: Feature]**
- **User Value**: {Why users need this}
- **Business Value**: {Impact on metrics/revenue}
- **Effort Estimate**: {Rough size - S/M/L/XL}

### Technical Debt Impact **[CONDITIONAL: Tech Debt]**
- **Current Problems**: {What's difficult/slow/buggy now}
- **Benefits of Fixing**: {What improves after refactoring}
- **Risk Assessment**: {Risks of not addressing this}

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] `MappingOp` extended with **`Cast { field, to }`, `Filter { predicate }`, `Compute { field, expr }`**
  (in `weir-connector-types`, kept on `ConfiguredStream.mapping`).
- [ ] A **bounded expression AST** (field refs, literals, comparison/boolean/arithmetic, a small safe fn set)
  + evaluator — no arbitrary code / no network / no I/O (dbt-boundary guard).
- [ ] **Engine applies `MappingSpec`** between read and write: row-JSON path (eval per record) and Arrow path
  (projection for select/drop, filter mask) — the mapping runs in `weir-engine`, not the connector.
- [ ] `Filter` drops non-matching records (counted); `Cast` coerces types (failed cast → dead-letter, not
  silent drop); `Compute` adds/overwrites a field from the expression.
- [ ] Unit tests for each op + the evaluator (incl. cast-failure → dead-letter, filter drop count); workspace
  green + clippy clean. Order-of-ops within a `MappingSpec` is deterministic + documented.

## Test Cases **[CONDITIONAL: Testing Task]**

{Delete unless this is a testing task}

### Test Case 1: {Test Case Name}
- **Test ID**: TC-001
- **Preconditions**: {What must be true before testing}
- **Steps**:
  1. {Step 1}
  2. {Step 2}
  3. {Step 3}
- **Expected Results**: {What should happen}
- **Actual Results**: {To be filled during execution}
- **Status**: {Pass/Fail/Blocked}

### Test Case 2: {Test Case Name}
- **Test ID**: TC-002
- **Preconditions**: {What must be true before testing}
- **Steps**:
  1. {Step 1}
  2. {Step 2}
- **Expected Results**: {What should happen}
- **Actual Results**: {To be filled during execution}
- **Status**: {Pass/Fail/Blocked}

## Documentation Sections **[CONDITIONAL: Documentation Task]**

{Delete unless this is a documentation task}

### User Guide Content
- **Feature Description**: {What this feature does and why it's useful}
- **Prerequisites**: {What users need before using this feature}
- **Step-by-Step Instructions**:
  1. {Step 1 with screenshots/examples}
  2. {Step 2 with screenshots/examples}
  3. {Step 3 with screenshots/examples}

### Troubleshooting Guide
- **Common Issue 1**: {Problem description and solution}
- **Common Issue 2**: {Problem description and solution}
- **Error Messages**: {List of error messages and what they mean}

### API Documentation **[CONDITIONAL: API Documentation]**
- **Endpoint**: {API endpoint description}
- **Parameters**: {Required and optional parameters}
- **Example Request**: {Code example}
- **Example Response**: {Expected response format}

## Implementation Notes **[CONDITIONAL: Technical Task]**

{Keep for technical tasks, delete for non-technical. Technical details, approach, or important considerations}

### Technical Approach
{How this will be implemented}

### Dependencies
{Other tasks or systems this depends on}

### Risk Considerations
{Technical risks and mitigation strategies}

## Status Updates **[REQUIRED]**

### 2026-06-24 — started; design finding + plan (no code landed yet)
**Current state:** `MappingOp` = `Select`/`Drop`/`Rename` only; the engine does **not** apply it
("passthrough mapping stub", weir-engine:9). So S1 = add ops + actually apply them.

**CRITICAL finding (drives the approach):** `MappingSpec` rides `ConfiguredStream`, which is in
`ReadContext` → it **crosses the wasm boundary**, so `MappingOp` is part of the **WIT contract**:
1. New variants must be **flat / non-recursive** (no `Box<Expr>`) to stay `WitType`-safe → v0 uses
   **structured ops**, not a recursive expression tree (richer nested exprs = a later engine-side parse).
2. **Every guest redeclares `MappingOp`** in `weir_guest_types` (echo/slow/faulty/arrow-sink/rest/
   postgres = 6). Extending the host enum without matching all 6 → **WIT-hash mismatch → every wasm load
   fails.** So the ripple is host enum + **6 guest blocks (exact match)** + rebuild all guests, *then* the
   evaluator + engine wiring.

**Planned v0 ops (flat, WIT-safe):** `Cast { field, to: CastType(Str|Integer|Float|Boolean) }` (fail →
dead-letter); `Filter { field, op: CompareOp(Eq|Ne|Lt|Le|Gt|Ge), value: String }` (numeric compare when
both parse, else string; non-match dropped + counted); `Compute { field, value: ComputeExpr(Const|Field|
Concat|Lower|Upper) }`. (Prototyped these host enums, then **reverted to keep the tree green** pending the
guest ripple.) → **NOTE: revise the AC above — v0 is flat structured ops, not a recursive AST.**

**Implementation order (next pass):** (1) extend host `MappingOp` + add `CastType`/`CompareOp`/`ComputeExpr`.
(2) Mirror into all 6 guest `weir_guest_types` blocks; rebuild guests (WIT must match). (3) `weir-engine::
mapping` evaluator `apply(&MappingSpec, record) -> Result<Option<Value>, DeadLetterReason>` (None=filtered).
(4) Apply in the sync loop between read+write — Rows (per-record JSON) + Arrow (projection/filter); dbt
boundary by construction. (5) Tests: each op + cast-fail→dead-letter + filter drop-count + deterministic
op order. Workspace + guests green, clippy.

### 2026-06-24 — DONE
Executed the full pass:
1. **Host** (`weir-connector-types`): `MappingOp` += `Cast`/`Filter`/`Compute` + `CastType`/`CompareOp`/
   `ComputeExpr` (flat, WIT-safe).
2. **All 6 guests** mirrored (echo/slow/faulty/arrow-sink/rest/postgres) + rebuilt — **WIT matches**
   (verified by wasm load + full suite).
3. **`weir-engine::mapping`** evaluator: `apply(&MappingSpec, record) -> Mapped{Keep|Filtered|DeadLetter}`;
   numeric-or-string compare; failed cast → dead-letter; flat compute (const/field/concat/lower/upper).
4. **Wired** via `map_batch` at the `Records` arm — **Rows** path (per-record JSON); **Arrow** passes
   through (Arrow-native projection/filter = follow-on). dbt boundary by construction (no I/O/eval). Added
   `serde_json` dep to weir-engine.
5. **Tests:** 4 evaluator unit tests + `wasm_mapping_filter_through_engine` (5 rows → filter n>3 → 2 written,
   end-to-end through the engine). **45 groups / 57 tests green, clippy clean.**

**Scope notes (vs original AC):** v0 is **flat structured ops**, not a recursive AST — forced by the WIT
contract. Deferred to a later slice: `Filter` is a single `field op value` (AND = chain Filters; **OR/
nesting** not yet); `Compute` has no arithmetic (concat/case/copy/const only); Arrow batches pass through.
These are the "richer expressions" follow-on (engine-side string-parse).
