---
id: cdc-via-postgres-logical
level: task
title: "CDC via Postgres logical replication (LSN resume)"
short_code: "WEIR-T-0028"
created_at: 2026-06-20T14:51:11.986656+00:00
updated_at: 2026-06-20T14:58:57.031409+00:00
parent: WEIR-I-0004
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0004
---

# CDC via Postgres logical replication (LSN resume)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0004]]

## Objective **[REQUIRED]**

Final slice of [[WEIR-I-0004]]: honor **`SyncMode::Cdc`** via Postgres **logical replication**, with the WAL position carried in the contract's **opaque** state (`StreamState.opaque`) — so, like the other slices, the connector does the honoring (the engine already commits `opaque` per chunk).

**Approach (v0, SQL-only — no replication-protocol connection):**
- Harness Postgres runs with **`wal_level=logical`** (docker-compose `command:` override).
- The Postgres source, on `Cdc`, ensures a durable **logical replication slot** (`test_decoding`, built-in — no extra plugin install), name derived deterministically from the stream + carried in `opaque`.
- Reads changes via `pg_logical_slot_get_changes(slot, NULL, NULL)` — a normal SQL call that **consumes + advances** the slot, so the slot itself is the durable resume point. Each change row → a record (`{lsn, data}` JSON); the last LSN is surfaced on the cursor for visibility.
- On resume, the same slot (from `opaque`) continues from where it left off → only new changes.

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

- [ ] docker-compose Postgres runs at `wal_level=logical`.
- [ ] Postgres source honors `SyncMode::Cdc`: ensures a `test_decoding` slot, reads via `pg_logical_slot_get_changes`, emits change records, carries the slot + LSN in `StreamState.opaque`.
- [ ] Integration test (`#[ignore]`, Postgres): first read after slot creation is empty; after INSERTs a read returns those changes; a further read after more INSERTs returns **only the new** changes (LSN/slot resume across reads).
- [ ] clippy clean; workspace green (ignored skipped). [[WEIR-S-0006]]/[[WEIR-S-0004]] note CDC implemented.

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

**2026-06-20 — DONE (verified live).** docker-compose Postgres now runs `wal_level=logical`. Added `cdc_slot_name` + `cdc_read` (ensures a `test_decoding` slot, consumes via `pg_logical_slot_get_changes`, emits `{lsn,data}` rows) and a `Cdc` branch in the source's `read` that carries the slot in `StreamState.opaque` + surfaces the last LSN on the cursor. Committed `d978619`. **Live test green** (`cdc_streams_changes_and_resumes`): slot-create captures nothing → 2 inserts captured → next read returns only the 1 new change (slot/LSN resume). Full PG suite + 55 workspace groups green; clippy clean. Specs [[WEIR-S-0006]]/[[WEIR-S-0004]] updated with the implementation status.
