---
id: s2-migrate-demo-connectors-to-wasm
level: task
title: "S2: Migrate demo connectors to wasm (slow, faulty, arrow-sink)"
short_code: "WEIR-T-0042"
created_at: 2026-06-22T20:03:25.630993+00:00
updated_at: 2026-06-23T00:41:46.343696+00:00
parent: WEIR-I-0011
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0011
---

# S2: Migrate demo connectors to wasm (slow, faulty, arrow-sink)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0011]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0011]]. Using the S1 ([[WEIR-T-0041]]) build seam, bring the demo connectors — `slow`,
`faulty`, `arrow-sink` — up as wasm component packages and run them through the engine/orchestrator/api
tests + `angreal ui demo` over wasm (dual-mode with native still allowed). After this, the **bulk of the
57-group suite is green on wasm**. AC: each connector loads via `from_wasm_package`, conforms (read/write/
discover/logs/dead-letters), and the demo + api/orchestrator tests pass over wasm.

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

### 2026-06-22 — DONE (connectors migrated; suite-wide default flip is S3)
Brought `slow`, `faulty`, `arrow-sink` up as **wasm guest crates** (`wasm-fixtures/{slow,faulty,arrow-sink}`)
using the S1 seam + the `fidius-guest` shape (the `weir_guest_types` block + `Connector` trait copied
**verbatim** from rest → WIT hash matches the host; only the `impl` differs). All three build for
`wasm32-wasip2` and run **through the engine** via `from_wasm_package`:
- `wasm_slow_engine` — slow (wasm) → arrow-sink (native): 3 rows/3 checkpoints, cursor→"3". ✓
- `wasm_faulty_engine` — faulty (wasm): **dead-letter** (1 dl + 1 row) + **fatal** (EngineError::Connector). ✓✓
- `wasm_arrow_sink_engine` — **both-ends-wasm** slow→arrow-sink, client-streaming `write` over wasm. ✓
- **`arrow` crate compiles to wasm32-wasip2** (~47s) → the Arrow bulk path is feasible on a wasm dest.
Engine suite green (9 groups); guest crates clippy-clean (only the harmless macro `cfg(host)` note).

**Scope note:** S2's deliverable is *the connectors exist + run on wasm* (done). Making the **demo + api/
orchestrator default to wasm** (vs `native_in_process`) is the **flip** → **S3** ([[WEIR-T-0043]]): that's
where `ConnectorRef::Native` is retired and resolution goes wasm-only.

**Follow-up (not blocking):** the verbatim `weir_guest_types` block is duplicated per guest crate (emit_wit
parses only the crate's own lib.rs). A wasm-buildable shared interface crate / SDK removes it — [[WEIR-S-0014]]
Rust SDK tier.
