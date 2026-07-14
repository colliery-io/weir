---
id: s4-stream-discovery-in-the-ui
level: task
title: "S4: Stream discovery in the UI (discover() endpoint + stream dropdown)"
short_code: "WEIR-T-0050"
created_at: 2026-06-24T00:05:54.750239+00:00
updated_at: 2026-06-24T01:56:59.568220+00:00
parent: WEIR-I-0010
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0010
---

# S4: Stream discovery in the UI (discover() endpoint + stream dropdown)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0010]]

## Objective **[REQUIRED]**

S4 of [[WEIR-I-0010]] (closes the initiative). Add **stream discovery** to the connection flow: a
`discover()` endpoint for the selected+configured source connector (kept **live** — `discover()` is
config-dependent, not snapshotted) + a **stream dropdown** in the connection form populated from it.

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

- [ ] `discover()` endpoint — given a configured source (`(name,version)` + config), loads the connector
  and returns its live stream catalog (not snapshotted).
- [ ] UI: a **stream dropdown** in the connection form populated from `discover()` for the
  selected+configured source.
- [ ] Stream selection flows into the `ConfiguredStream.stream` for the connection.
- [ ] Graceful handling of `discover()` failure (bad config / unreachable source) — surfaced in the form,
  no crash.

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

### 2026-06-24 — DONE (commit de44371)
- **`ConnectorRef::discover(config)`** (orchestrator) — resolve + `handle.discover` → `DiscoverOutcome`.
- **`App::discover_streams(plugin, config)`** (weir-app) — maps `DiscoverOutcome::Catalog` → stream names;
  `Error` → `AppError::Config` (graceful failure).
- **`POST /connectors/{plugin}/discover`** (weir-api) — body = config JSON → `Json<Vec<String>>`.
- **UI** (weir-ui): the `stream` field is now a `<select>` over a live discover `use_resource` (keyed on
  `source()`, `config.peek()` so it doesn't refetch per keystroke), falling back to a text input when
  discovery returns nothing. Selection flows into the connection's stream. Compiles for wasm32.
- **Live-smoked:** `POST /connectors/slow/discover` → `["slow"]`, `/connectors/echo/discover` → `["echo"]`.
  44 groups + clippy clean. Redeployed via `angreal ui demo` restart.

**Closes [[WEIR-I-0010]]** (S1–S4 all done). Deferred follow-ups (logged on [[WEIR-T-0049]]): exact
`(name,version)` pinning on connection-create + an unregister button in the UI.
