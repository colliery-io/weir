---
id: weir-serve-daemon-scheduler-worker
level: task
title: "weir serve daemon (scheduler + worker loop) + single-node E2E"
short_code: "WEIR-T-0015"
created_at: 2026-06-19T02:36:52.428555+00:00
updated_at: 2026-06-19T02:46:04.847931+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# weir serve daemon (scheduler + worker loop) + single-node E2E

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

`weir serve` — the single-node **daemon**: an async loop that ticks the [`Scheduler`] (registering schedules from `connections` with an interval) and runs the [`Worker`] to drain the relay, on one in-process agent (no broker). Plus a single-node **end-to-end** test: configure a scheduled connection, run the loop, assert the pipeline executed.

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

- [x] `App::register_schedules` registers interval `connections` with the `Scheduler`; `App::serve` ticks the scheduler + drains the worker in one `tokio::select!` loop.
- [x] Graceful shutdown: `serve` takes a shutdown future; `main` wires `ctrl_c`, breaking the loop cleanly.
- [x] Test (`tests/serve.rs`, injected clock): a scheduled connection fires + runs end-to-end (committed outbox) — single-node, no broker; not-due before the interval.

## Status Updates

- **2026-06-19 — DONE.** `weir serve` daemon: `Scheduler` (SystemClock) + `Worker` in a `select!` loop (tick → `run_until_idle`) until a shutdown future (ctrl-c) resolves; schedules sourced from interval `connections`. Test drives the building blocks against a `ManualClock` (no sleeping) and asserts the pipeline ran via `Store::outbox_count`. `scheduler.tick()` runs inline (brief sync query); a `spawn_blocking` wrap is a trivial later refinement. **Slice 4 (single-node deploy) complete.** Full workspace green (47 groups).

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
