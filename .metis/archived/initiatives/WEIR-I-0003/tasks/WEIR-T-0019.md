---
id: reconcile-run-tracking-runmanager
level: task
title: "Reconcile run tracking (RunManager into the relay)"
short_code: "WEIR-T-0019"
created_at: 2026-06-19T13:46:00.446034+00:00
updated_at: 2026-06-19T16:02:54.775237+00:00
parent: WEIR-I-0003
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0003
---

# Reconcile run tracking (RunManager into the relay)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0003]]

## Objective **[REQUIRED]**

Run status lives in two places: `weir-engine::RunManager` (its own `runs` table) and the orchestrator's `work_units` (the relay). The API/UI read `work_units`; the engine writes `runs`. They can disagree. Make `work_units` the single source of truth — retire the `runs` table or reduce `RunManager` to a thin reader/bridge over the relay — so the engine, CLI, API, and UI all reflect one run record.

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

- [x] One run record — relay `work_units`. The parallel `runs` table + `RunManager`/`RunRecord`/`RetryPolicy` were **removed** (they were unused by the live path).
- [x] No regressions: full workspace green (50 groups); `weir run` / `weir api` history+feed unaffected (they already read `work_units`).
- [x] Divergence is now structurally impossible (single record); the `run_manager` test was deleted (its retry coverage is in the orchestrator tests).

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

- **2026-06-19 — DONE.** Investigation showed `RunManager` (engine `runs` table) was used only by its own test — the live path (`App` → relay → `Worker` → `work_units`) never touched it, and retry semantics live in the orchestrator. Removed `RunManager`/`RunRecord`/`RetryPolicy`, the `runs` DDL, `EngineError::NotFound`, `now_ms`, the now-dead `std::time`/`ErrorKind` imports, and the `run_manager` test. Relay `work_units` is the single run record. Full workspace green (50 groups).
