---
id: incremental-sync-postgres-source
level: task
title: "Incremental sync: Postgres source honors SyncMode + cursor"
short_code: "WEIR-T-0026"
created_at: 2026-06-19T18:22:21.257011+00:00
updated_at: 2026-06-19T18:26:28.450634+00:00
parent: WEIR-I-0004
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0004
---

# Incremental sync: Postgres source honors SyncMode + cursor

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0004]]

## Objective **[REQUIRED]**

Slice 2 of [[WEIR-I-0004]]: a destination existed; now add the **Postgres source** that honors `SyncMode` + the incremental cursor. The engine already conveys `sync_mode` + `cursor_field` (in `ReadContext.stream`) + the committed cursor (`ReadContext.state.cursor`) and commits `next_state` — so, like write modes, the **source** does the honoring:
- `FullRefresh` — `SELECT row_to_json(t) FROM table` (all rows, every run; ignores the cursor).
- `Incremental` — `… WHERE "cursor_field" > $cursor ORDER BY "cursor_field"`, emitting rows + `next_state.cursor = max(cursor_field)`, so the next run resumes.

**v0 cursor limitation:** compared lexically (`"col"::text > $1`), so the cursor column must be text-orderable (ISO timestamps / text / zero-padded). Richer cursor typing is a follow-on.

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

- [x] `weir-connector-postgres` gained the Source role: `read` honors `sync_mode` + `cursor_field` + `state.cursor`; `FullRefresh` reads all (`row_to_json`), `Incremental` reads past the cursor and returns the new max. `spec` now `[Source, Destination]`.
- [x] `tests/incremental.rs` (`#[ignore]`, real Postgres): incremental first-run reads all + sets cursor → resumes reading only the new row → empty when nothing new; full-refresh ignores the cursor. All green via the harness.
- [x] No engine change (engine conveys sync_mode/cursor + commits next_state); clippy clean; workspace green (53 groups).

## Status Updates

- **2026-06-19 — DONE.** Postgres source honors `SyncMode`. `read_records()` (pure, testable): `FullRefresh` → `SELECT row_to_json(t)`; `Incremental` → `… WHERE "cf"::text > $1 ORDER BY "cf"`, next cursor = last (ASC) value. v0 lexical cursor (text-orderable columns) noted. Verified `up → 2 source tests green → down`. Branch `feat/weir-i-0004-full-contract` (commit `12a6485`).

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
