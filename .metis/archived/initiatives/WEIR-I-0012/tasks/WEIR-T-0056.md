---
id: s3-onboarding-api-import-manifest
level: task
title: "S3: Onboarding API — import {manifest} (instant) + widen /catalog/available"
short_code: "WEIR-T-0056"
created_at: 2026-06-24T19:51:44.624210+00:00
updated_at: 2026-06-25T00:18:36.014812+00:00
parent: WEIR-I-0012
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0012
---

# S3: Onboarding API — import {manifest} (instant) + widen /catalog/available

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0012]]

## Objective **[REQUIRED]**

S3 of [[WEIR-I-0012]] — the **onboarding API** ([[WEIR-S-0015]]). Extend `POST /catalog/import` (`ImportDto`)
with a `manifest: Option<String>` variant that registers a manifest connector **instantly** (no compile),
alongside the existing `path` (crate, compiles) / `package` (folder). Widen `GET /catalog/available` to list
the **vendored manifests + crates** (the "discover & select" source). This is the thin HTTP layer over the
S1 rails + the S2 preview.

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

- [ ] `ImportDto` gains `manifest: Option<String>`; `POST /catalog/import {manifest}` registers instantly
  (no compile) and returns the `CatalogEntry`. `path`/`package` unchanged.
- [ ] `GET /catalog/available` returns **both** vendored manifests and crates, tagged by kind.
- [ ] Errors surface (NFR-1.4): bad manifest → 4xx with the importer's reason; never a half-registered row.
- [ ] API test: import a manifest → it appears in `GET /catalog` as a `Manifest`-kind entry. Workspace green.

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

### 2026-06-24 — DONE (commit cde1b48)
- `ImportDto` += `manifest` + `name`; `catalog_import` matches `(path | package | manifest)` →
  `Source::Manifest{yaml,name}` (instant register, no compile). path/package unchanged.
- `available_packages` now returns **tagged** `AvailablePackage{name, kind}` — scans `connectors_dir` (kind
  `crate`) + a `manifests/` dir (`*.yaml`/`*.yml` → kind `manifest`, T-0058 populates it); `GET
  /catalog/available` returns it. Added `manifests_dir()` helper.
- `AppError::Config → 400` so a bad manifest is a client error with the importer's reason surfaced.
- **Test** `import_manifest_via_api`: POST manifest → 200 kind=manifest, lists in `/catalog`; garbage → 400.
  **45 groups / 60 tests green, clippy clean.**

**Note:** the `/catalog/available` response shape changed `Vec<String>` → `Vec<AvailablePackage>`; the UI's
`fetch_available` is updated in T-0057 (the workspace stays green; the UI builds separately via trunk).
