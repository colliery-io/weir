---
id: s2-weir-app-catalog-connections
level: task
title: "S2: weir-app catalog/connections onto the portable schema + typed query builder"
short_code: "WEIR-T-0060"
created_at: 2026-06-25T03:09:14.681814+00:00
updated_at: 2026-06-25T11:30:29.012749+00:00
parent: WEIR-I-0013
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0013
---

# S2: weir-app catalog/connections onto the portable schema + typed query builder

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0013]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0013]] — convert weir-app's catalog/connections store (`connectors`,
`connections` tables, 13 `sql_query` sites) onto the portable surface established in S1
([[WEIR-T-0059]]): add its tables to the logical migration, regenerate, run schema via
the dispatched runner in `App::open`, and rewrite all DML on the typed query builder.

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

## Acceptance Criteria **[REQUIRED]**

- [ ] weir-app's tables (`connectors`, `connections`) are in the logical migration set;
  generated schema + per-backend migrations regenerated.
- [ ] `App::open` creates schema via the dispatched runner — no raw dialect DDL in weir-app.
- [ ] All 13 weir-app `sql_query` sites use the typed query builder + portable types;
  `dispatch` only where unavoidable.
- [ ] weir-app round-trips on Postgres AND the SQLite suite stays green; dual-backend test
  coverage for the catalog + connection round-trip.

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

### 2026-06-25 — COMPLETE (commit 41e6c4c). All ACs met.
- `connections` + `connectors` added to the logical migration; regenerated. **`every_secs`
  DOUBLE PRECISION → REAL** (the generator has no DOUBLE/f64; f32 is exact for schedule
  intervals — the app casts f64↔f32 at the query boundary, public `Connection` keeps f64).
- `App::open` **drops all raw DDL** (incl. the best-effort kind/manifest ALTERs) — `Store::open
  → weir_schema::migrate` now creates every table.
- **All 13 weir-app `sql_query` → typed builder** (0 left). Upserts (`INSERT OR REPLACE`) →
  portable **update-then-insert**; `register_connector`'s update now **preserves `created_at`**.
  `ConnRow`/`CatalogRow` `QueryableByName` structs → `ConnTuple`/`CatalogTuple` typed loads +
  the existing mappers. **No `dispatch` needed.**
- **Robustness:** `weir_schema::migrate` wrapped in a transaction + a **Postgres advisory lock**
  (`pg_advisory_xact_lock`) so the non-idempotent generated DDL applies exactly once under
  concurrent migrators (parallel tests sharing a Postgres; future multi-process weir).
- **Verify:** `dual_backend.rs` grows `app_tables_round_trip` — **4 tests green on BOTH backends
  in parallel** (pg via `angreal integration`). weir-app sqlite suite green; weir-engine green;
  workspace builds.
