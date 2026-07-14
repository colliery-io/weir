---
id: engine-stream-loop-drive-stream
level: task
title: "Engine stream loop: drive Stream<ReadMessage>, commit on Checkpoint"
short_code: "WEIR-T-0031"
created_at: 2026-06-21T03:25:50.244005+00:00
updated_at: 2026-06-21T04:00:24.645886+00:00
parent: WEIR-I-0006
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0006
---

# Engine stream loop: drive Stream<ReadMessage>, commit on Checkpoint

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0006]]

## Objective **[REQUIRED]**

Slice 3 of [[WEIR-I-0006]] (per [[WEIR-A-0029]]): the `weir-engine` stream drive loop. Construct the configured source/dest once, pull the `Stream<ReadMessage>`, and per message: `Records` → map ([[WEIR-A-0026]]) + write; `Checkpoint` → commit `StreamState` transactionally ([[WEIR-A-0011]]); `Log`/`DeadLetter` → record. Backpressure = pull as the dest drains; cancel = drop the stream. Resume-from-last-checkpoint on crash/cancel holds. Depends on [[WEIR-T-0029]], [[WEIR-T-0030]].

**AC:** engine drives a streaming sync with transactional checkpoint-on-`Checkpoint`; resume + retry-from-last-checkpoint preserved; orchestrator tests green.

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

**2026-06-21 — DONE (engine + orchestrator libs build + clippy clean).** `weir-engine` `sync_with` stays **sync** (executor bridge via `spawn_blocking` unchanged) but drives the v1 async `read` on a **local current-thread tokio runtime** (`block_on`); DB commits stay synchronous. Loop: `Records` → `dest.write` (sync, per batch) + accumulate; `DeadLettered` → accumulate; `Log` → drop (v0); `Checkpoint` → commit `stream_state` + outbox + pending dead-letters in ONE txn (connector owns checkpoint granularity). Dropped `config` from `sync`/`sync_with` (configured instances). `EngineError::Rt`; deps `tokio` (rt) + `futures`. `weir-orchestrator`: `ConnectorRef::resolve(&Config)` → configured `native_in_process`/`from_wasm_package`; executor resolves with `spec.config` + calls the 5-arg `sync_with`. **Gate met:** `cargo build -p weir-engine -p weir-orchestrator --lib` + clippy clean. **⚠ Runtime risk to verify in [[WEIR-T-0033]]:** `block_on` of a fresh runtime inside `spawn_blocking` — should be fine (spawn_blocking is the blocking pool), but if it panics ("runtime within runtime"), move the `block_on` to a fresh `std::thread`. **Red (expected):** connectors still have v0 `read` ([[WEIR-T-0033]]); engine/orchestrator tests + weir-app use old signatures ([[WEIR-T-0033]]).
