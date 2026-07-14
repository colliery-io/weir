---
id: s5-vendored-manifest-corpus
level: task
title: "S5: Vendored manifest corpus — curated in-repo manifests/ surfaced in discover & select"
short_code: "WEIR-T-0058"
created_at: 2026-06-24T19:52:02.959842+00:00
updated_at: 2026-06-25T00:22:55.642033+00:00
parent: WEIR-I-0012
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0012
---

# S5: Vendored manifest corpus — curated in-repo manifests/ surfaced in discover & select

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0012]]

## Objective **[REQUIRED]**

S5 of [[WEIR-I-0012]] — the **vendored manifest corpus** ([[WEIR-S-0015]]), the phase-1 stand-in for a
registry. A curated in-repo `manifests/` directory of permissive declarative manifests (each authored
solely from its API's official documentation) that "discover & select" lists. Provenance/license
recorded per manifest ([[WEIR-A-0018]] / [[WEIR-V-0001]] IP-clean).

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

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] An in-repo `manifests/` set of a few curated permissive declarative manifests (authored from official API docs).
- [ ] Each carries provenance + license metadata; an excluded/unknown-license manifest never lands.
- [ ] `GET /catalog/available` surfaces them in "discover & select" (depends on S3/T-0056).
- [ ] At least one onboards + runs end-to-end through the rails (depends on S1/T-0054).

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

### 2026-06-24 — DONE (commit df96426)
`manifests/` dir = the phase-1 "discover & select" stand-in for a registry:
- `coinpaprika.yaml` (`/v1/coins`, no auth, top-level array), `rickandmorty.yaml` (`/character`, `results[]`,
  page pagination) — both **authored from public API docs** (Apache-2.0), with provenance headers.
- `README.md` records provenance + license per manifest.
- `available_packages` (T-0056) surfaces them with `kind=manifest`.
- **Test** `vendored_manifests_list_and_onboard`: both appear in `available_packages` + onboard as
  `kind=manifest`. **45 groups / 61 tests green.**

A connection on one runs end-to-end via the T-0054 rails (rest + manifest config) — the same path
`rick-live` exercised live in the demo.
