---
id: s4-close-fresh-weir-migrates
level: task
title: "S4: Close — fresh weir migrates + serves on Postgres; compose demo flipped to the SoR"
short_code: "WEIR-T-0062"
created_at: 2026-06-25T03:09:19.056403+00:00
updated_at: 2026-06-25T12:06:40.595605+00:00
parent: WEIR-I-0013
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0013
---

# S4: Close — fresh weir migrates + serves on Postgres; compose demo flipped to the SoR

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0013]]

## Objective **[REQUIRED]**

S4 of [[WEIR-I-0013]] — close the loop: a fresh `weir` migrates and serves end-to-end on
Postgres (the SoR), and `compose.demo.yml` is flipped back from the sqlite interim to the
Postgres service and verified. Lock dual-backend coverage in so the store can't silently
regress to SQLite-only.

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

- [ ] `weir --db postgres://… api` on an empty database creates all schema and serves;
  onboard a manifest + run a connection end-to-end against Postgres.
- [ ] `compose.demo.yml` runs weir against the Postgres service (the sqlite default is no
  longer the demo path); `angreal docker up` comes up healthy and serves the control plane.
- [ ] A both-backends test pass (`#[diesel_dualdb::test(pg, sqlite)]`) covers the
  control-plane store across the three crates; `angreal test` green on SQLite, the Postgres
  path green under the integration compose.
- [ ] No raw SQLite-dialect `sql_query` remains in the control plane (grep clean); the
  weir-engine "SQLite-only / portable path is later" note is removed.

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

### 2026-06-25 — COMPLETE (commit 63d64cf). All ACs met; initiative closed.
- **Fresh `weir --db postgres://… api` on an empty pg**: migrates + serves +
  `POST /catalog/import {manifest_name: coinpaprika}` → **HTTP 200**, `/catalog` reads it
  back. Schema creation through onboarding, all on Postgres (proven directly).
- **`compose.demo.yml` on the Postgres SoR**: `docker compose up` (built from the Dockerfile,
  `WEIR_DB=postgres://…`) comes up healthy — **`/catalog/available` = 9 connectors on pg**.
  The original `type "blob" does not exist` crash is fixed end-to-end. `angreal docker up/down`.
- **Dual-backend coverage**: `weir-schema/tests/dual_backend.rs` — 3 tests × {pg, sqlite} = **6
  green** across the engine, app, and orchestrator tables (control_plane / app_tables /
  work_units claim).
- **Control plane grep-clean**: 0 raw `sql_query` in weir-engine/app/orchestrator (weir-schema's
  3 are the dispatched migrate runner). weir-engine "SQLite-only / portable path is later" note
  removed (S1).
- **`angreal test` (full sqlite suite)**: 26 groups / 50 green; the one failure is the
  pre-existing load-flaky `partitioned_plan_...` (passes isolated — testkit staging race, not
  this work).
- **Edge note:** the generated DDL isn't `IF NOT EXISTS`, so migrate assumes a fresh db (the
  `__weir_schema_version` sentinel + pg advisory lock handle re-runs + concurrency, but not a
  *partial* schema left by a different/older migrator). A stale demo volume needed
  `compose down -v`. Hardening (IF-NOT-EXISTS generated DDL) is a possible follow-up.
