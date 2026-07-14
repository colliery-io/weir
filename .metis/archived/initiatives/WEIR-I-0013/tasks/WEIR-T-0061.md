---
id: s3-weir-orchestrator-work-unit
level: task
title: "S3: weir-orchestrator work-unit queue onto the portable schema + typed query builder"
short_code: "WEIR-T-0061"
created_at: 2026-06-25T03:09:16.602103+00:00
updated_at: 2026-06-25T11:45:56.828515+00:00
parent: WEIR-I-0013
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0013
---

# S3: weir-orchestrator work-unit queue onto the portable schema + typed query builder

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0013]]

## Objective **[REQUIRED]**

S3 of [[WEIR-I-0013]] — convert weir-orchestrator's work-unit queue, the largest surface
(26 `sql_query` sites), onto the portable schema + typed query builder. The claiming /
queue semantics (the cloacina-style claim pattern) are the likely `dispatch` hot-spots —
handle any backend-divergent claim statement explicitly and exhaustively, keeping the
existing concurrency/claiming tests green.

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

- [ ] weir-orchestrator's queue tables are in the logical migration set; generated schema
  + per-backend migrations regenerated.
- [ ] Schema created via the dispatched runner; no raw dialect DDL remains.
- [ ] All 26 `sql_query` sites use the typed query builder + portable types; any
  backend-divergent claim/queue statement is isolated in a documented
  `DualConnection::dispatch`.
- [ ] Orchestration round-trips on Postgres (scheduler → claim → run → outbox) AND the
  SQLite suite (incl. the concurrency/claiming tests) stays green; a dual-backend test
  covers the claim path.

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

### 2026-06-25 — COMPLETE (commit 07e09b1). All ACs met.
- `work_units` + `schedules` in the logical migration; regenerated. **`id BIGINT`,
  client-generated monotonic** (`now_ms() << 20 | counter`) — preserves the FIFO
  `ORDER BY id` claim + the run feed without rowid autoincrement; `last_insert_rowid()`
  dropped (plan/add/add_cron return the generated id).
- **All 26 `sql_query` → typed builder (0 left).** The **claim needs no `dispatch`**: it's a
  portable `SELECT` (lowest-id due pending) + a **state-guarded `UPDATE`** whose rows-affected
  check IS the atomic lock — no `FOR UPDATE`/`SKIP LOCKED`. Guarded UPDATEs (lease, heartbeat,
  progress, mark_done/failed, requeue, reclaim_expired), `recent`/`history`/`has_active`
  (`eq_any`), and schedules CRUD all typed; column arithmetic `attempt = attempt + 1` via the
  builder. 6 `QueryableByName` row structs → tuple loads + `SpecTuple`/`RunTuple` aliases.
- `Relay::new` + `Scheduler::new` drop their `CREATE TABLE` — `Store::open → weir_schema::migrate`.
- **Verify:** orchestrator **15 sqlite tests green incl. concurrency/claiming**;
  `dual_backend.rs work_units_claim_round_trips` passes on **BOTH** backends (pg via
  `angreal integration`); workspace builds. `partition` (a pg keyword) is accepted unquoted.
