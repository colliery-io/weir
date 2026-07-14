---
id: run-orchestration-state-machine
level: task
title: "Run orchestration: state machine + outbox relay + executor seam"
short_code: "WEIR-T-0011"
created_at: 2026-06-18T17:05:24.101438+00:00
updated_at: 2026-06-18T17:12:30.124325+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Run orchestration: state machine + outbox relay + executor seam

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Build the run/work-unit **orchestration** layer between triggers and the engine ([[WEIR-S-0004]], realizing [[WEIR-A-0010]]/[[WEIR-A-0011]], async boundary per [[WEIR-A-0028]]). New `weir-orchestrator` crate (only place tokio lives): a `work_units` queue + state machine, a **claim/heartbeat-lease relay** (DB-as-queue, no broker; agent-fleet-ready), and the cloacina-mapped `WorkExecutor` seam (`InProcessExecutor` now, remote later) over the sync `Engine` via `spawn_blocking`. Retry relocates here as a state transition. Manual triggers, scheduler ([[WEIR-T-0009]]), and backfill ([[WEIR-T-0010]]) become thin `plan()` callers.

### Design (state machine + seam)
- **States:** `Run: Queued → Running → {Succeeded | Failed}`. `WorkUnit: Pending → Leased → {Done | Failed}`, or `→ Pending(next_attempt_at)` on transient.
- **Seam (ported from cloacina `dispatcher`/`executor`):** lightweight `WorkReadyEvent { work_unit_id, attempt }` (context lazy-loaded from the store); async `WorkExecutor::execute(event) -> Result<ExecutionResult, ExecutorError>` (`ExecutionResult = Completed | Failed{kind} | Retry | Skipped`); `ExecutorConfig { max_concurrent, task_timeout, heartbeat_interval }`.
- **Relay:** `plan(spec)` enqueues a `Pending` unit; `claim(owner)` atomically leases the next due unit (`Skipped` if another claimant won); `reclaim_expired()` returns dead-worker leases to `Pending`. `InProcessExecutor` resolves `ConnectorRef`s from the row, runs `Engine::sync` in `spawn_blocking`; never holds a `DualConnection` across `.await`.

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

- [x] `weir-orchestrator` crate (tokio confined here): async `WorkExecutor` trait + `WorkReadyEvent` (IDs, lazy context load) + `ExecutionResult` + `WorkerConfig`, ported from cloacina's `dispatcher`/`executor`.
- [x] `work_units` table + `Relay`: `plan()` enqueues; `claim(owner, lease)` atomically leases the next due unit (txn + conditional update; loser gets `None`); `reclaim_expired()` recovers expired leases; states `Pending/Leased/Done/Failed` + `next_attempt_at`.
- [x] `InProcessExecutor` runs the sync `Engine` via `spawn_blocking`, resolving `ConnectorRef`s from the row; no `DualConnection` held across `.await`.
- [x] Retry is a transition (`Failed{Transient}` → `Pending(next_attempt_at)` with backoff); fatal/config fail fast.
- [x] 6 `#[tokio::test]`s: drain→Done; transient blip recovers; persistent transient exhausts→Failed; fatal fails (no retry); expired lease reclaimed; atomic claim issues once. *(loser gets `None` = idle; `Skipped` is reserved for load-after-claim-gone.)*

## Status Updates

- **2026-06-18 — DONE.** Built `weir-orchestrator` per [[WEIR-A-0028]]: async boundary lives here, sync `Engine` bridged via `spawn_blocking`. `Relay` is the `work_units` queue + state machine (sync DB on `Store::pool()`); `WorkExecutor` is the cloacina-mapped seam (`InProcessExecutor` now, remote later — `ConnectorRef` is serializable and resolved executor-side, so a handle never crosses the seam). `Worker` loops claim→execute→apply; retry moved off `RunManager` into a relay transition. Full workspace green (38 groups). Follow-ons: `Dispatcher` router (multi-executor) + heartbeat-during-execution are deferred (single executor, lease-on-claim is enough for single-node v0); `RunManager`↔`Relay` reconciliation (runs vs work_units) when the scheduler lands.

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
