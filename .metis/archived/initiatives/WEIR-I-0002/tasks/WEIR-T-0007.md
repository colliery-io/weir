---
id: run-lifecycle-runmanager-manual
level: task
title: "Run lifecycle + RunManager (manual trigger, persisted run status)"
short_code: "WEIR-T-0007"
created_at: 2026-06-18T13:35:19.558134+00:00
updated_at: 2026-06-18T13:43:57.866326+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Run lifecycle + RunManager (manual trigger, persisted run status)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

A **RunManager** that records each sync as a durable `run` (queued → running → succeeded/failed, with timestamps + row counts) in the `diesel-dualdb` store, and a **manual trigger** that invokes the engine ([[WEIR-T-0005]]) and persists the outcome. Foundation for the scheduler ([[WEIR-T-0009]]), retries ([[WEIR-T-0008]]), and backfill ([[WEIR-T-0010]]).

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

- [ ] `runs` table carries lifecycle status, started/finished timestamps, chunks + rows_written, and an error message on failure.
- [ ] `RunManager::trigger(connection, stream)` creates a run row, invokes the engine, and durably transitions the run to succeeded/failed.
- [ ] Run history is queryable (list runs for a connection; latest run).
- [ ] Test: a manual trigger drives echo→engine→sink and the run row reflects success + row count; a failing run records the error.

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

- **2026-06-18 — DONE.** `weir-engine`: extended `runs` table (status/started/finished/chunks/rows_written/error) + `RunManager` (`trigger`/`get`/`runs`/`latest`). `trigger` records `running` → `succeeded`/`failed` durably, capturing engine errors in the record (returns `Ok`; only store errors `Err`). Added `weir-connector-faulty` (fails on `{"fail":true}` by returning a malformed envelope → engine surfaces a Codec error). `tests/run_manager.rs`: success (rows + history) + failure (error recorded), both green. `last_insert_rowid()` read on the same pooled connection as the insert; timestamps are epoch-ms via `SystemTime` (no chrono dep).
