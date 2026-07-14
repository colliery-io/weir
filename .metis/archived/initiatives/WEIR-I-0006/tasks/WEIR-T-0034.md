---
id: streaming-write-client-streaming
level: task
title: "Streaming write (client-streaming destination) [phasable]"
short_code: "WEIR-T-0034"
created_at: 2026-06-21T03:25:57.232668+00:00
updated_at: 2026-06-21T19:55:06.350225+00:00
parent: WEIR-I-0006
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0006
---

# Streaming write (client-streaming destination) [phasable]

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0006]]

## Objective **[REQUIRED]**

Slice 6 of [[WEIR-I-0006]] (per [[WEIR-A-0029]], **phasable**): streaming `write` — the destination consumes a `Stream<RecordBatch>` (client-streaming) instead of per-batch calls, returning a `WriteOutcome`/acks. Engine produces the batch stream; backpressure flows from the dest. Optional for v1 (read-streaming can ship first); symmetric is the target. Depends on [[WEIR-T-0029]]–[[WEIR-T-0031]].

**AC:** destination `write` is client-streaming; engine streams batches; arrow-sink + postgres dests ported; conformance green.

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

### 2026-06-21 — done (commit aab534e)
- **Contract:** `write` is now client-streaming — `fn write(&self, ctx: WriteContext,
  batches: Stream<RecordBatch>) -> WriteOutcome` (was `#[wire(raw)]` one-batch). The host produces
  the batches; the connector pulls them via `next_item()` (backpressure). `config` bound at construction.
- **WIT modeling:** `Result<WriteReceipt, ConnectorError>` can't cross WIT as a struct field (fidius
  maps `Result` only as a return, hardcoded to `plugin-error`), so added a `WriteResult` enum
  (`Ok|Err`, like `DiscoverOutcome`); `WriteOutcome`/`WriteReceipt`/`WriteResult` derive `WitType`.
  Removed the obsolete `WriteRequest`.
- **Engine:** buffers `Records` since the last checkpoint and writes them as **one client-stream per
  checkpoint segment**, then the atomic checkpoint commit — exactly-once-per-checkpoint preserved;
  backpressure flows within a segment.
- **Seam:** `ConnectorHandle::write(ctx, Vec<RecordBatch>)` over fidius `call_client_streaming` (sync).
- **Connectors:** echo/faulty/arrow-sink/postgres/slow + dylib & wasm **codegen** + the echo wasm
  fixture ported. Client-streamed items cross as bincode → the wasm guests' local `RecordBatch`/
  `ArrowIpc` derive `serde` (Deserialize) in addition to `WitType`.
- **Conformance:** full workspace suite green (56 groups, 0 failures) incl `wasm_seam`/`wasm_http`
  exercising client-streaming write E2E (native == wasm write outcome). clippy clean.

**AC met:** destination `write` is client-streaming; engine streams batches; arrow-sink + postgres
(+ echo/faulty/slow) dests ported; conformance green (dylib + wasm). (Live-Postgres integration is
`#[ignore]`/docker-gated — the pure `write_records` fn is unchanged.)
