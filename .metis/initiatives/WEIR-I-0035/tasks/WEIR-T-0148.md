---
id: bug-resident-start-stop-no-op-in
level: task
title: "BUG: resident Start/Stop no-op in the UI (missing tenant-scoped routes) + run_feed active-inclusion + HTTP request logging"
short_code: "WEIR-T-0148"
created_at: 2026-07-14T01:20:33.451057+00:00
updated_at: 2026-07-14T01:20:33.451057+00:00
parent: WEIR-I-0035
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# BUG: resident Start/Stop no-op in the UI (missing tenant-scoped routes) + run_feed active-inclusion + HTTP request logging

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0035]]

## Summary (COMPLETE, demo-verified)

Three UI/observability fixes surfaced by exercising the resident demo (user clicked Start/Stop; nothing moved):

1. **BUG — resident Start/Stop were no-ops in the UI (root cause).** The Leptos UI scopes calls to the active tenant
   (`localStorage.weir_active_tenant` → `apath` rewrites to `/tenants/{id}/…`). The tenant-scoped
   `/tenants/{id}/connections/{name}/start` + `/stop` **routes never existed** (only the un-scoped
   `/connections/{name}/start|stop` from F1.5). So the UI's POSTs fell through to the **SPA fallback → 200
   (index.html)** → the UI saw success → fired the toast → **but no start/stop ran** ("toast fires, tile never
   changes"). Fix: added `t_start`/`t_stop` handlers (`weir-api/src/lib.rs`, mirroring `t_run`) + their routes +
   `authz.rs` entries (`platform(Admin)`, like `t_run`). Verified: `POST /tenants/default/connections/resident-demo/
   stop → {"cancelled":2}` → pill stopped; `…/start → {"started":true}` → pill live.

2. **`run_feed` active-inclusion** (`weir-app/src/store.rs`) — was `ORDER BY id DESC LIMIT 50`; a long-lived
   resident's old-id run got crowded out of the window by frequently-firing tiles, so the UI showed a *running*
   resident as "stopped". Now unions in all active (`pending`/`leased`/`running`) units regardless of the cap. Active
   work is never hidden by a recency limit.

3. **Observability** — `weir-api` HTTP request-logging middleware (`method`/full-`path`/`status` at
   `debug`, target `weir_api::http`) + UI **immediate-refresh** after Start/Stop (refetch `/runs`+`/connections`
   instead of waiting for the 800ms poll). The request log is what pinpointed #1 (showed the browser POSTing
   `/tenants/default/…/stop` → 200 with no matching route).

Build/fmt/clippy green; `angreal ui build` green. Diagnosed via logs, not `/runs` polling (the recurring lesson).
Not committed.

## Objective **[REQUIRED]**

{Clear statement of what this task accomplishes}

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

*To be added during implementation*