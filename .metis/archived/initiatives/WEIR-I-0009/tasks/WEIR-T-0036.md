---
id: s2-capture-logs-diagnostics-to-a
level: task
title: "S2: Capture logs + diagnostics to a run_logs store"
short_code: "WEIR-T-0036"
created_at: 2026-06-22T13:02:03.334555+00:00
updated_at: 2026-06-22T13:30:40.724823+00:00
parent: WEIR-I-0009
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0009
---

# S2: Capture logs + diagnostics to a run_logs store

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0009]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0009]] (legibility). The engine currently **discards** connector logs (`ReadMessage::Log(_entry) => {}`) and write-path `diagnostics`. Capture them to a `run_logs` store (keyed per connection/stream, mirroring `dead_letters`), expose via `GET /connections/{name}/logs`, and emit a sample log from a demo connector so there's something to see. S6 renders them; S2 captures + exposes.

**Decision:** logs are written **immediately** as they arrive (not buffered to checkpoint like dead-letters) so they're visible mid-run and survive a `Fatal`; keyed per-connection (the engine doesn't hold the orchestrator's work-unit id — run-precise correlation is a later refinement, consistent with `dead_letters`).

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

- [ ] {Specific, testable requirement 1}
- [ ] {Specific, testable requirement 2}
- [ ] {Specific, testable requirement 3}

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

### 2026-06-22 — done (commit f0b04ae)
- **engine**: `run_logs` table; `ReadMessage::Log` written **immediately** (no longer dropped) +
  write-outcome `diagnostics` captured; `Store::logs(connection, limit)` read + serde `LogRecord` +
  `level_str`/`now_ms` helpers.
- **app**: `logs()` + re-export `LogRecord`.
- **api**: `GET /connections/{name}/logs`.
- **connector**: `Slow` now emits an Info log per read (`"slow source: slept …ms, emitting … rows"`)
  so the demo/test has something to show.
- **test**: `run_captures_connector_logs`; full suite green (57 groups), clippy clean.

**Deferred to S6:** rendering logs in the run-detail view (data is exposed via the API now).
