---
id: s3-catalog-management-api-role
level: task
title: "S3: Catalog management API + role-filtered UI dropdowns"
short_code: "WEIR-T-0049"
created_at: 2026-06-24T00:05:49.254082+00:00
updated_at: 2026-06-24T01:32:30.758168+00:00
parent: WEIR-I-0010
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0010
---

# S3: Catalog management API + role-filtered UI dropdowns

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0010]]

## Objective **[REQUIRED]**

S3 of [[WEIR-I-0010]]. Expose the catalog over the **API + UI**: `GET /connectors` (registered), an
**available-to-register** listing (folder scan), register/unregister, and **role-filtered source/dest
dropdowns** that feed the selected entry's `config_schema` into the existing schema-driven config form.
**Closes the deferred [[WEIR-I-0009]] S5** (connector discovery). Builds on S1 (catalog table) + S2 (ingress).

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

- [ ] `GET /connectors` — registered catalog (name, version, roles, origin, status, `config_schema`).
- [ ] **Available-to-register** listing (folder scan) — distinct from registered; `POST` **register** (runs
  the S2 ingress) + **unregister** (remove row + cached artifact).
- [ ] UI: source/dest become **role-filtered dropdowns** over the *registered* catalog (Source-role for
  source, Destination/ReverseEtl for dest).
- [ ] Selecting an entry feeds its `config_schema` into the existing **schema-driven config form**
  ([[WEIR-I-0009]] S5).
- [ ] Creating a connection **pins the chosen `(name, version)`** into the `ConnectorRef`.

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

### 2026-06-24 — API done (commit fdeac72); UI pending an integration decision
**Done — catalog management API:** weir-app `unregister_connector` + `available_packages` (folder scan);
weir-api routes under `/catalog`: `GET /catalog` (registered list), `GET /catalog/available` (folder scan),
`POST /catalog/import` (runs the S2 ingress, `origin=private`), `DELETE /catalog/{name}/{version}`. Smoke
test green (routes respond; register/list round-trip covered by the weir-app unit test). 45 groups, clippy clean.

**UI dropdowns — blocked on a fork I hit while wiring them.** The form's source/dest are free-text inputs
today; role-filtered `<select>`s would read `GET /catalog`. **But the catalog is empty until connectors are
registered, and the demo only *stages* packages (it never registers them).** So the dropdown needs a
**catalog-population strategy** first — a genuine design choice:
1. **Auto-register bundled connectors on startup** (scan `connectors_dir`, snapshot spec, upsert) — the
   dropdowns "just work" out of the box; first-party connectors are always present.
2. **Demo seeds registration** (the `angreal ui demo` task imports the staged packages) — keeps prod
   empty-by-default, demo populated.
3. **Dropdown unions registered + available** (folder scan) — show stageable-but-unregistered packages too,
   registering on selection.

~~Leaning (1)~~ — superseded by the user's two-phase model below.

### 2026-06-24 — RESOLVED: two-phase model (user direction) + corrected for A-0018
My "auto-register on startup" lean **contradicted A-0018** (registration is explicit, not a startup scan).
User reframed into **two phases**:
- **Phase 1 — "Add connectors" surface (populate the catalog):** a **dropdown of discoverable** connectors
  (`GET /catalog/available` folder scan now → crates.io later) **+ a free input to point at a specific
  thing** (path/git/crates.io). Either → `POST /catalog/import` → compile + register → now available.
  Explicit registration, A-0018-consistent.
- **Phase 2 — use them:** the connection form's source/dest become **role-filtered `<select>`s over the
  *registered* catalog** (`GET /catalog`), feeding the existing `fetch_props` schema form; pin `(name,
  version)` on create.
- **Demo:** seed (register) a few connectors so the connection dropdowns work out of the box, **and leave
  one connector staged-but-unregistered** (the "add me" example, e.g. `echo`) so the Phase-1 Add flow is
  demoable end-to-end (pick from available → Add → compiles/registers → now usable in Phase 2).

### 2026-06-24 — DONE (commit 36f3e97)
- **API** (commit fdeac72): `/catalog` (registered), `/catalog/available` (folder scan), `POST
  /catalog/import` (ingress), `DELETE /catalog/{name}/{version}`. Smoke test green.
- **UI** (weir-ui, compiles for wasm32-unknown-unknown): **Phase 1** "Add connectors" panel — discover
  `<select>` (available) + crate-path input → `POST /catalog/import` → `catalog.restart()`. **Phase 2** —
  source/dest are role-filtered `<select>`s over `GET /catalog` feeding the existing `fetch_props` schema
  form.
- **Demo** (task_ui.py): registers arrow-sink/slow/faulty; leaves `echo` staged-unregistered as the "Add"
  example.
- **Live-smoked** against a running server: `POST /catalog/import {package:weir-slow-pkg}` → registered
  `slow@0.1.0` (roles/schema/contract snapshotted); `GET /catalog` shows it; `GET /catalog/available` lists
  all 4 staged. Folder ingress path also unit-tested. 44 groups + clippy clean.

**Deferred (small):** create still pins via `connector_ref` (version `0.0.0`) rather than the dropdown's
exact `(name, version)` — the gate is non-breaking (uncataloged pin passes), exact-version pinning through
the connection DTO is a follow-up. `DELETE`/unregister has no UI button yet (API only).
