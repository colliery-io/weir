---
id: s5-streaming-throughput-validation
level: task
title: "S5: Streaming throughput validation (wasm vs cdylib)"
short_code: "WEIR-T-0045"
created_at: 2026-06-22T20:03:48.145143+00:00
updated_at: 2026-06-23T20:17:17.620194+00:00
parent: WEIR-I-0011
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0011
---

# S5: Streaming throughput validation (wasm vs cdylib)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0011]]

## Objective **[REQUIRED]**

S5 of [[WEIR-I-0011]] — close the loop. Benchmark **streaming throughput** of a connector on wasm vs the
(retired) cdylib path to confirm [[WEIR-A-0030]]'s premise: wasm ≥ cdylib on the v1 streaming surface
([[WEIR-A-0029]]). Capture the numbers in the ADR record. AC: a reproducible benchmark (rows/s over a
representative read→write) with wasm-vs-cdylib results recorded against A-0030; no regression (ideally a
win). If wasm regresses, that's a material finding to surface, not bury.

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

### 2026-06-23 — DONE: streaming throughput benchmark recorded (commit 6a322ef)
**No wasm-vs-cdylib A/B** (user direction): S3 retired the cdylib path, and the contract itself moved
unary→streaming ([[WEIR-A-0029]]), so an apples-to-apples comparison isn't meaningful. S5 instead records a
**streaming throughput baseline** for the wasm path.

**Method:** added a bulk-batch mode to the slow guest (`batch:true` → one `Records` + one `Checkpoint`,
isolating the streaming surface from per-row checkpoint commits); `wasm_throughput` bench (`#[ignore]`): a
**500,000-row** batch flows **wasm source → engine → wasm arrow-sink** (both wasm boundaries: read pull +
client-streamed write). Run: `cargo test --release -p weir-engine --test wasm_throughput -- --ignored --nocapture`.

**Result (release):** 500k rows in **191 ms = ~2.6M rows/s (30.8 MB/s)** end-to-end. (Debug host: ~650k
rows/s.) Healthy — the wasm streaming path moves data at multi-million rows/s through both sandbox
boundaries; no pathological regression from the flip. Baseline-of-record for [[WEIR-A-0030]].
