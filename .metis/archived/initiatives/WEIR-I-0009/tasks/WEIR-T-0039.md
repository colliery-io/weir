---
id: s4-configurable-editable
level: task
title: "S4: Configurable + editable connections (config, schedule, edit/delete, error surfacing)"
short_code: "WEIR-T-0039"
created_at: 2026-06-22T13:02:11.121639+00:00
updated_at: 2026-06-22T14:30:41.354887+00:00
parent: WEIR-I-0009
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0009
---

# S4: Configurable + editable connections (config, schedule, edit/delete, error surfacing)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0009]]

## Objective **[REQUIRED]**

S4 of [[WEIR-I-0009]] (pillar A, config IN). The create form sent only `{name,source,dest,stream}` — so a connection couldn't be configured (no base_url/creds/DB-URL, no schedule), edited, or deleted, and failures were swallowed. Add a **config (JSON) field** + **schedule (every_secs)**, **edit** (prefill from a card; create is upsert) + **delete** per card, and **error surfacing**. No backend change — the API/DTO already carried config/`every_secs`/`cron` and a DELETE route.

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

### 2026-06-22 — done (commit 7228951)
- **Form**: config (JSON) `textarea` (parsed + validated, surfaces JSON errors) + `every (secs)` field;
  `NewConnection` now sends `config`/`every_secs`/`cron`. "Save connection" (upsert).
- **Cards**: `edit` (prefills the form from the connection — config stringified, schedule), `del`
  (DELETE), `Run` — all `stop_propagation`; the card body opens the run-detail.
- **Errors surfaced**: create/run/delete failures set an `error` signal → a `.form-err` banner
  (previously `let _ = …` swallowed them).
- UI `Connection` carries `config`/`cron` for the edit prefill; `serde_json` added to `weir-ui`.
- **Verified**: create with `config={rows:4,sleep_ms:500}` + `every_secs:30` round-trips intact;
  delete → 204. UI builds (trunk). (No backend Rust change; workspace suite still green.)
