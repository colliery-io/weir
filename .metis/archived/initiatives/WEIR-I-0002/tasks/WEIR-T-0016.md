---
id: control-plane-http-api-connection
level: task
title: "Control-plane HTTP API (connection CRUD + run + history)"
short_code: "WEIR-T-0016"
created_at: 2026-06-19T02:49:13.417289+00:00
updated_at: 2026-06-19T02:55:20.301955+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Control-plane HTTP API (connection CRUD + run + history)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

The **control-plane HTTP API** ([[WEIR-S-0002]] / [[WEIR-A-0006]]): an `axum` server over the existing `weir_cli::App`, exposing the connection-centric config model ([[WEIR-A-0007]]) + run control. New `weir-api` crate (router as a lib, testable via `oneshot` — no bound port) wired into a `weir api` subcommand. The config model itself already exists (Slice 4's `connections`); this is the remote surface.

### Endpoints (v0)
- `GET /health`
- `GET /connections`, `POST /connections`, `GET /connections/:name`, `DELETE /connections/:name`
- `POST /connections/:name/run` → plan + drain, returns the run outcome
- `GET /connections/:name/runs` → work-unit history (id/state/attempt)

JSON DTOs use plugin-name strings for source/dest (`ConnectorRef::Native`) + raw-JSON `config`, mirroring the CLI.

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

- [x] `axum` router over `Arc<App>` (`weir_cli::api`) with connection CRUD + run + history endpoints + JSON DTOs. *(In `weir-cli`, not a separate `weir-api` crate, to avoid a `weir-cli`↔`weir-api` dependency cycle.)*
- [x] `App` gains `delete_connection` + `history` (work-unit list via `Relay::history` + `WorkUnitStatus`).
- [x] `weir api --port` subcommand serves the router; `api::router(app)` is testable without a port.
- [x] `tests/api.rs` (`oneshot`): create→list→run (→ done)→history; 404 on missing.

## Status Updates

- **2026-06-19 — DONE.** `weir-cli::api`: axum 0.8 router over `Arc<App>` (`GET /health`, connection CRUD, `POST .../run`, `GET .../runs`). DTOs use plugin-name strings + raw-JSON config. `CliError`→HTTP (NotFound→404, else 500). Folded into `weir-cli` rather than a `weir-api` crate because the bin needs the API and the API needs `App` → a separate crate would cycle (a future `weir-app` extraction would let it split cleanly). Tested via `tower::ServiceExt::oneshot` (no bound port). **Slice 3 complete.** Full workspace green (48 groups).

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
