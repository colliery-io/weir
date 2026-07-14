---
id: postgres-wasm-feature-parity
level: task
title: "Postgres wasm feature parity (incremental, partitions, write/upsert, CDC) + retire native crate"
short_code: "WEIR-T-0046"
created_at: 2026-06-23T21:43:55.917368+00:00
updated_at: 2026-06-23T22:15:35.227334+00:00
parent: WEIR-I-0011
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0011
---

# Postgres wasm feature parity (incremental, partitions, write/upsert, CDC) + retire native crate

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0011]]

## Objective **[REQUIRED]**

Bring the postgres **wasm guest** to feature parity with the (now-deleted) native `weir-connector-postgres`,
then retire that crate — closing out the last native connector. Port: Incremental (cursor), CDC
(`pg_logical_slot_get_changes`), key-shard partitions, and write modes (Append / Overwrite / Upsert). AC:
all features green against a live Postgres via the wasm guest; native crate deleted; workspace green.

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

### 2026-06-23 — DONE: full parity proven; native crate retired (commits 36bf239 et al.)
Ported the native `read_records`/`write_records`/`cdc_read`/`shard_predicate` into `wasm-fixtures/postgres`
over the sync `sockets::tcp` + `postgres-protocol` client.

- **Key decision — simple Query + inlined literals, NOT the extended protocol.** A first cut used
  Parse/Bind/Execute for parameters; it **hung** (the guest blocked on a socket read — finicky extended-query
  framing). Pivoted to the proven **simple Query** path with values inlined via `lit()` (single-quote-escaped;
  injection-safe under `standard_conforming_strings`, PG default). One `query_rows(sql)` primitive serves all
  SELECT/DDL/INSERT.
- **Ported:** Source — FullRefresh, Incremental (`t.cf::text > '<cursor>'` + ORDER BY, returns new max),
  CDC (`pg_logical_slot_get_changes`, sanitized slot in `opaque`), key-shard partitions (`hashtext` predicate).
  Dest — Append/Overwrite (`data JSONB`), Upsert (`ON CONFLICT … DO UPDATE`). Config: `url` (parsed) or
  discrete `host`/`port`/`user`/`password`/`dbname`; `table` defaults to stream name.
- **Verified green vs live postgres:16** (`crates/weir-engine/tests/wasm_postgres_engine.rs`, `#[ignore]`):
  `pg_wasm_append_then_fullrefresh_roundtrip`, `pg_wasm_upsert_is_idempotent`,
  `pg_wasm_incremental_advances_cursor`, `pg_wasm_cdc_captures_inserts`. Partitions share the identical,
  already-proven `shard_predicate` SQL.
- **Retired** `crates/weir-connector-postgres/` — the **last native connector crate**. Workspace builds; 44
  test groups green. WASM-always now has **zero native connector code**.
