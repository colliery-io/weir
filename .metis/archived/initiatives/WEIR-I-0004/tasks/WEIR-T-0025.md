---
id: write-modes-postgres-destination
level: task
title: "Write modes: Postgres destination honors Append/Upsert/Overwrite"
short_code: "WEIR-T-0025"
created_at: 2026-06-19T18:07:18.539610+00:00
updated_at: 2026-06-19T18:15:09.447524+00:00
parent: WEIR-I-0004
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0004
---

# Write modes: Postgres destination honors Append/Upsert/Overwrite

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0004]]

## Objective **[REQUIRED]**

Slice 1 of [[WEIR-I-0004]]: make a destination **honor `WriteMode`**. The engine already conveys `write_mode` + `primary_key` to the destination (via `WriteContext.stream`); the reference sink ignores it. Implement the real **`weir-connector-postgres` destination** honoring all three modes against the slice-0 harness Postgres:
- `Append` — `INSERT`.
- `Upsert { business_keys }` — `INSERT … ON CONFLICT (keys) DO UPDATE` (idempotent under at-least-once re-delivery).
- `Overwrite` — truncate-then-insert.

Records are `RecordBatch::Rows` (JSON text); v0 stores each row as a `JSONB` `data` column (+ business-key text columns forming the PK for upsert).

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

- [x] `weir-connector-postgres` is a real `Connector` destination: `write` honors `ctx.stream.write_mode` (`Append`/`Upsert{business_keys}`/`Overwrite`); `spec` declares Destination; `check` pings. Pure `write_records()` does the SQL.
- [x] `tests/write_modes.rs` (`#[ignore]`, real harness Postgres): **upsert idempotent + updates** (re-deliver → count stable, key update reflected), **append accumulates** (×2 → doubles), **overwrite replaces**. All green via the harness.
- [x] No engine change (engine already conveys `write_mode`); clippy clean; `cargo test --workspace` green (52 groups, ignored skipped).

## Status Updates

- **2026-06-19 — DONE.** Postgres destination honors `WriteMode`. Found `$1::jsonb` made Postgres infer the param as jsonb (rust-postgres serializes `String` as text → "error serializing parameter 0"); fixed with `$1::text::jsonb`. Rows → `JSONB data` column; upsert keys → `TEXT` PK columns. Verified `angreal integration up → 3 write-mode tests green → down`. Branch `feat/weir-i-0004-full-contract` (commit `db88b78`).

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
