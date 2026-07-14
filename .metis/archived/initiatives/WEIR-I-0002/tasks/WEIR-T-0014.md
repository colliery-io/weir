---
id: connection-config-model-single
level: task
title: "Connection config model + single-node weir CLI (init/add/run)"
short_code: "WEIR-T-0014"
created_at: 2026-06-19T02:36:43.535438+00:00
updated_at: 2026-06-19T02:43:46.300462+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Connection config model + single-node weir CLI (init/add/run)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

Turn the library crates into a runnable product: a **single-node `weir` binary** (`weir-cli`, bin `weir`) over embedded SQLite, no broker ([[WEIR-S-0012]] / NFR-DP-1). Includes the **minimal connection config model** (the useful slice of [[WEIR-A-0007]]/[[WEIR-S-0002]] that the binary needs) so a connection is persisted and runnable. **Flips the initiative exit criterion "a real pipeline runs end-to-end on single-node."**

### Scope
- `connections` table (name, source/dest `ConnectorRef`, stream, config, optional schedule secs) on the shared `Store`.
- CLI (clap): `weir init [--db]`, `weir connection add --name --source --dest --stream [--config] [--every]`, `weir connection list`, `weir run --name` (one-shot: `Relay::plan` → `Worker::run_until_idle`, prints the work-unit outcome).
- Links the reference connectors (echo/arrow-sink/rest/faulty) so they resolve by name; `ConnectorRef::Native` now, `Wasm` later.
- CLI logic in a lib (thin `main`) so it's testable without shelling out.

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

- [ ] `connections` table + `Connection` model persisted on the `Store` (add/list/get).
- [ ] `weir` binary with `init`/`connection add`/`connection list`/`run` over embedded SQLite; CLI logic in a lib.
- [ ] `weir run --name` drives a configured connection through the orchestrator (plan → drain) and reports the outcome (work-unit done + rows).
- [ ] Test: add a connection (echo→arrow-sink) + run it against a temp DB → work unit `done`, rows delivered — a real single-node pipeline.

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
