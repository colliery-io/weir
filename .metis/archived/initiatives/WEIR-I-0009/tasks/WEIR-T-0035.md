---
id: s1-expose-persisted-run-state
level: task
title: "S1: Expose persisted run state (failure reason, dead-letter detail, cursor, progress)"
short_code: "WEIR-T-0035"
created_at: 2026-06-22T13:01:59.352321+00:00
updated_at: 2026-06-22T13:19:00.477959+00:00
parent: WEIR-I-0009
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0009
---

# S1: Expose persisted run state (failure reason, dead-letter detail, cursor, progress)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0009]]

## Objective **[REQUIRED]**

S1 of [[WEIR-I-0009]] (legibility, pillar B). Surface the run state weir already persists but hides, so a failed/dead-lettering run explains itself. Specifically: expose the **failure reason** (`work_units.error`), **dead-letter detail** (`record` + `reason`, not just a count), the **resume cursor** (`stream_state`), and **chunk progress** (`outbox`) through the API; render the failure reason in the UI feed/cards. No engine changes (logs are S2). Visible in `angreal ui demo`: `failing-api` shows *why*, `dead-letters` shows *what*.

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

- [ ] `RunRow` (`/runs`) + history (`/connections/{name}/runs`) include the failure `error` (nullable) — orchestrator already stores it via `mark_failed`.
- [ ] A dead-letters endpoint returns per-record `{record, reason, stream}` (paginated), per connection and/or per run; the engine's `dead_letters` table gains a queryable read path (today only `dead_letter_count`).
- [ ] Run/connection state exposes the resume `cursor` (from `stream_state`) and committed `chunks` (from `outbox`).
- [ ] UI: failed feed rows + connection cards show the failure reason inline (e.g. `failed · simulated fatal failure`); a dead-letter count links to the detail.
- [ ] `weir-api` tests cover the new fields/endpoint; workspace + clippy green; verified live in `angreal ui demo` (`failing-api` shows why; `dead-letters` shows what).

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

### 2026-06-22 — done (commit c07e330)
- **orchestrator**: `RunRow` (`/runs`) + `WorkUnitStatus` (history) now carry the failure `error`
  (read from the existing `work_units.error`).
- **engine**: `Store::dead_letters(connection, limit)` list read (`stream`/`record`/`reason`) +
  `serde` on `DeadLetterRecord`; reused existing `cursor()` + `outbox_count()`.
- **app**: `dead_letters()` + `connection_state()` (`{cursor, chunks, dead_lettered}`) + `ConnectionState`.
- **api**: `GET /connections/{name}/dead-letters` + `GET /connections/{name}/state`.
- **ui**: failed feed rows + cards show the reason inline (`failed · <error>`).
- **test**: `failed_run_surfaces_error_and_dead_letters`; full suite green (57 groups), clippy clean.
- **Verified live**: demo `/runs` returns `error="simulated fatal failure"` for `failing-api`.

**Deferred to S6 (run-detail view):** rich UI rendering of the dead-letter list + resume cursor/chunks
(the *data* is exposed via the API now; S6 builds the view). Live mid-run progress is S3.
