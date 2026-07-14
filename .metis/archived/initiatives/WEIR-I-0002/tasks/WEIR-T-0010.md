---
id: backfill-bounded-historical-re-sync
level: task
title: "Backfill: bounded historical re-sync"
short_code: "WEIR-T-0010"
created_at: 2026-06-18T13:35:35.801250+00:00
updated_at: 2026-06-18T18:16:56.599923+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Backfill: bounded historical re-sync

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

**Bounded historical backfill**: re-sync a stream over an explicit cursor window (from/to) by overriding the resume `StreamState` for the backfill run, **without corrupting the live incremental cursor**. Backfill runs are recorded distinctly in run history.

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

- [x] `Relay::plan_backfill(spec, from_cursor)` enqueues a backfill that starts from the window lower bound (`seed_cursor`), not the live cursor — via `Engine::sync_with` + `SyncOptions{ state_key, seed_cursor }`.
- [x] The live incremental `stream_state` cursor is **preserved/untouched** — the backfill checkpoints under an isolated `state_key` (`backfill:<stream>:<from>`); the connector still reads the real stream.
- [x] Backfill runs are distinguishable (their `work_units` row carries `state_key`/`seed_cursor`; the checkpoint key is prefixed `backfill:`).
- [x] Test (`tests/backfill.rs`): a normal run sets the live cursor to `LIVE-9`; a backfill runs under its own key (`BF-1`); the live cursor is asserted unchanged.

## Status Updates

- **2026-06-18 — DONE.** Backfill = checkpoint isolation. `Engine::sync_with(SyncOptions{state_key, seed_cursor})` (plain `sync` delegates with defaults); state-load + checkpoint/outbox/dead-letter writes key on `state_key` while the connector still sees the real `stream`. `Relay::plan_backfill` sets `state_key="backfill:<stream>:<from>"` + `seed_cursor=from`; `WorkSpec.{state_key,seed_cursor}` persisted as `work_units` columns. `Store::cursor()` + faulty `emit_cursor` make the isolation test deterministic. Upper-bound (`to`) enforcement is connector-dependent (deferred). **Slice 2 complete (5/5).** Full workspace green (40 groups).

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
