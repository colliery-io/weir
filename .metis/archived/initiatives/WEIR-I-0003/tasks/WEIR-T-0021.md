---
id: extract-weir-app-standalone-weir
level: task
title: "Extract weir-app + standalone weir-api crate"
short_code: "WEIR-T-0021"
created_at: 2026-06-19T13:46:13.394714+00:00
updated_at: 2026-06-19T16:18:49.331390+00:00
parent: WEIR-I-0003
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0003
---

# Extract weir-app + standalone weir-api crate

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0003]]

## Objective **[REQUIRED]**

Resolve the Phase-1 dependency cycle (the control-plane API was folded into `weir-cli` because the bin needs the API and the API needs `App`). Extract `App` + the connection config model into a new **`weir-app`** crate, and the axum router into a standalone **`weir-api`** crate over `weir-app`. `weir-cli` becomes a thin binary depending on both. Connectors still force-link/resolve by name and the Dioxus UI stays embedded.

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

- [x] `weir-app` crate holds `App`/`Connection`/config + run/drain/serve (`AppError`, no axum); `weir-api` crate holds the axum router + serve + embedded UI (build.rs moved here) over `weir-app`.
- [x] `weir-cli` is a thin bin (no lib) over `weir-app` + `weir-api`; force-links the reference connectors; UI still embedded + served (`weir 0.0.1` smoke-ok).
- [x] No dependency cycle; full workspace green (52 groups); tests relocated — cli/serve → weir-app, api oneshot → weir-api.

## Status Updates

- **2026-06-19 — DONE.** Split `weir-cli`'s lib into `weir-app` (application core, `CliError`→`AppError`) and `weir-api` (axum + UI). `weir-cli` is now bin-only and depends on both, force-linking connectors in `main.rs`. `build.rs` (UI embed) moved to `weir-api`. Tests moved with connector force-links added (they previously inherited them from the cli lib). Confirmed integration tests can name the crate's regular deps. Full workspace green (52 groups), binary smoke-ok.

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
