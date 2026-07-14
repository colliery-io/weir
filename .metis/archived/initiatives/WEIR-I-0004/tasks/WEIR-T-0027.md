---
id: partitioned-parallel-reads-fan-out
level: task
title: "Partitioned/parallel reads: fan-out + per-partition checkpoints"
short_code: "WEIR-T-0027"
created_at: 2026-06-20T13:19:52.233762+00:00
updated_at: 2026-06-20T14:45:53.664853+00:00
parent: WEIR-I-0004
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0004
---

# Partitioned/parallel reads: fan-out + per-partition checkpoints

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0004]]

## Objective **[REQUIRED]**

Slice 3 of [[WEIR-I-0004]]: **partitioned/parallel reads**. **No ADR needed** — the contract already models partitioning (`Partition { id, bounds }`, `PartitionScheme`, `ReadContext.partition`, `StreamInfo.partitioning`); the engine just hard-codes a single `p0`. So this is engine/orchestrator *honoring* it (same pattern as S1/S2):
- `materialize_partitions(scheme)` → `Vec<Partition>` (v0: `Unpartitioned` + `ByKeyShards{key,count}`).
- `WorkSpec` carries a `Partition`; a partitioned plan **fans out N work units** (one per partition), each with a **per-partition `state_key`** so checkpoints are independent; the worker(s) run them in parallel.
- Engine `sync_with` passes `spec.partition` into `ReadContext.partition`.
- The **Postgres source honors `Partition`** (`ByKeyShards` → `WHERE hashtext(key) % count = shard`), so each unit reads a disjoint slice.

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

- [ ] `materialize_partitions(&PartitionScheme) -> Vec<Partition>` (Unpartitioned → 1; ByKeyShards → N); `WorkSpec.partition`; `Relay::plan_partitioned(spec, partitions)` fans out N units with per-partition `state_key`; engine `sync_with` uses `spec.partition`.
- [ ] Orchestrator test: a `ByKeyShards{count:N}` plan creates N work units that drain in parallel, each checkpointing under its own `state_key` (independent cursors).
- [ ] Integration test (`#[ignore]`, Postgres): the Postgres source honors `Partition` (`ByKeyShards`) so the N units read **disjoint** rows whose union = the whole table.
- [ ] No contract/ADR change; clippy clean; workspace green (ignored skipped).

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

**2026-06-20 — DONE (verified live).** No ADR needed — the contract already modeled partitions. Implemented `materialize_partitions` (Unpartitioned + ByKeyShards), `WorkSpec.partition` (+ work_units column), `Relay::plan_partitioned` (fan-out, per-partition `state_key`), engine `sync_with` threads `spec.partition`, and the Postgres source honors a key-shard `Partition` (`hashtext` modulo). Committed `54c9138`. **Tests green:** orchestrator fan-out (`ByKeyShards{3}` → 3 units, distinct state_keys, all drain done) + **live Postgres** `key_shards_read_disjoint_and_cover_all` (4 shards, disjoint, union = all rows); S1/S2 PG tests still pass. clippy clean.
