---
id: s5-connector-stream-discovery
level: task
title: "S5: Connector + stream discovery"
short_code: "WEIR-T-0040"
created_at: 2026-06-22T13:02:13.244100+00:00
updated_at: 2026-06-22T14:54:04.672641+00:00
parent: WEIR-I-0009
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0009
---

# S5: Connector + stream discovery

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0009]]

## Objective **[REQUIRED]**

S5 of [[WEIR-I-0009]] — **reshaped** (2026-06-22, user direction) from "discovery dropdowns" to a
**config contract**: connectors declare a config schema and the UI renders it into a form. Decided
**JSON Schema** (reuse the existing `ConnectorSpec.config_schema`; Airbyte-aligned — their
`connectionSpecification` is JSON Schema, so [[WEIR-I-0008]] parity configs map 1:1). ADR later.

**Deferred (needs a small fidius FR — re-export the in-process registry through the white-label
facade):** source/dest **enumeration dropdowns** + `discover()`-based **stream dropdowns**. The by-name
spec endpoint covers the contract render without enumeration.

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

### 2026-06-22 — config-contract render done (commit 8b23402)
- **connectors**: meaningful `config_schema` (JSON Schema) — `faulty` fleshed out (fail/token/until/
  dead_letter); `slow`/`postgres` already had one; codegen still emits `{"type":"object"}` (follow-up).
- **orchestrator**: `ConnectorRef::spec()` (loads w/ empty config → `ConnectorSpec`).
- **app/api**: `App::connector_spec` + `GET /connectors/{plugin}/spec`.
- **ui**: fetches the selected source's spec, parses `config_schema.properties`, renders typed fields
  (text/number; password for `format:password`/`airbyte_secret`) editing the config; raw-JSON textarea
  fallback when no schema. Verified `/connectors/{Slow,Faulty}/spec` serve their schemas. 57 green, clippy clean.

### Deferred → follow-up
- **Enumeration dropdowns + stream discovery** need a fidius FR: re-export `fidius_core::registry`
  (the in-process descriptor registry) through the white-label facade so the host can list connectors.
  Tracked as a follow-up; the config-contract render (the reshaped ask) is delivered.
