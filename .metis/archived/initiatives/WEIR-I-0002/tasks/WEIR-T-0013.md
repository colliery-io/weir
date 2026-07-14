---
id: migration-fidelity-e2e-airbyte
level: task
title: "Migration fidelity E2E: Airbyte YAML to a running connector"
short_code: "WEIR-T-0013"
created_at: 2026-06-19T02:13:00.276767+00:00
updated_at: 2026-06-19T02:23:57.818400+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Migration fidelity E2E: Airbyte YAML to a running connector

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

The **migration fidelity gate** ([[WEIR-A-0020]]): an Airbyte declarative manifest, run through the importer ([[WEIR-T-0012]]) → `weir_manifest::Manifest` → codegen ([[WEIR-T-0003]]) → a generated `fidius` connector that **paginates a mock API end-to-end** (reusing the `rest` harness). Proves Airbyte YAML → running weir connector with no hand-editing.

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

- [x] An Airbyte declarative manifest → importer → `Manifest` → `weir-codegen` produces a connector — proven **byte-identical** to `codegen(rest)`.
- [x] That connector paginates a mock HTTP server + advances its incremental cursor — it *is* the `weir-connector-rest` that `weir-engine/tests/rest.rs` runs end-to-end (composition with a green E2E).
- [x] `tests/fidelity.rs` asserts the import→codegen output equals the rest codegen output, with no manual edits (`manifests/airbyte-rest.yaml`).

## Status Updates

- **2026-06-19 — DONE.** Migration fidelity proven by composition: `codegen(import(airbyte-rest.yaml))` is byte-for-byte equal to `codegen(rest.yaml)`, and `rest.rs` already compiles + paginates that exact connector against a mock. Chose byte-equality of generated source over a temp-dir recompile (fast, robust; codegen ignores the fields that differ — nullability, `record_selector` None-vs-empty). **Slice 5 (migration importer) complete.** Full workspace green (43 groups).

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
