---
id: retry-policy-with-backoff-dead
level: task
title: "Retry policy with backoff + dead-letter"
short_code: "WEIR-T-0008"
created_at: 2026-06-18T13:35:24.883161+00:00
updated_at: 2026-06-18T15:03:26.854564+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Retry policy with backoff + dead-letter

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Layer **retry-with-backoff** on the connector error taxonomy ([[WEIR-A-0014]] §5): **transient** → retry with bounded, exponentially-backed-off attempts; **config/fatal** → fail fast; **record-level** → dead-letter and continue. Realizes at-least-once ([[WEIR-A-0011]]) via idempotent re-runs from the committed checkpoint.

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

- [x] `RetryPolicy { max_attempts, base_delay }` (exponential backoff) re-executes a run on **transient** failure, resuming from the committed checkpoint (the engine re-reads state each attempt → no duplicates beyond idempotent). `runs.attempts` recorded.
- [x] **Config/fatal** fail immediately (no retry); **record-level** is dead-lettered without failing the run — via the new three-channel envelope (`dead_letters` operational channel; engine persists to a `dead_letters` table atomically with the checkpoint; `runs.dead_lettered`).
- [x] Exhausted retries mark the run `failed` with the last error.
- [x] Tests (`weir-engine/tests/run_manager.rs`): transient blip recovers (succeeds on attempt 3); persistent transient exhausts (attempts == cap, failed); fatal does not retry (attempts == 1); dead-letter recorded while run succeeds.

## Status Updates

- **2026-06-18 — DONE.** Preceded by the **contract refinement** (committed separately): `read`/`write` envelopes became three channels — operational (`next_state`/`has_more`/`diagnostics`/`dead_letters`, always present) + data⊕error as `Result<_, ConnectorError>`. Built `RetryPolicy` + `RunManager::with_policy`; retry branches on `ConnectorError.kind == Transient`. `weir-connector-faulty` is now stateful with a **required token** (namespaced global counter) so retry/recovery is deterministic + test-isolated; `until` makes a transient blip recover. Record-level dead-letters land in the operational channel (not the `Err` arm — partial success), persisted atomically with the checkpoint. Full workspace green.

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

*To be added during implementation*
