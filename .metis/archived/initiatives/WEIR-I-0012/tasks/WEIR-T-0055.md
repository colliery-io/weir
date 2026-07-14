---
id: s2-manifest-preview-gate-importer
level: task
title: "S2: Manifest preview gate — importer fidelity report + POST /catalog/preview"
short_code: "WEIR-T-0055"
created_at: 2026-06-24T19:51:35.144281+00:00
updated_at: 2026-06-25T00:11:22.375002+00:00
parent: WEIR-I-0012
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0012
---

# S2: Manifest preview gate — importer fidelity report + POST /catalog/preview

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0012]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0012]] — the **preview gate** ([[WEIR-S-0015]] REQ-1.4, [[WEIR-A-0020]]). `weir-importer` emits
a **fidelity report** for a manifest — tier + confidence + streams + **unsupported features** — and
`POST /catalog/preview { manifest }` returns it **synchronously, without running anything**. Onboarding shows
it before commit so we **never onboard a silently-broken connector**: an operator sees exactly what a
manifest will and won't do (given the shared runtime's *current* capability) ahead of time.

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

- [ ] `weir-importer` exposes a fidelity report: `{ tier, confidence, streams[], unsupported[] }` for a manifest.
- [ ] `POST /catalog/preview { manifest }` returns it synchronously — **no compile, no run, no registration**.
- [ ] Unsupported declarative features are **named** in `unsupported[]` (not silently dropped).
- [ ] Tier/confidence derive from what the **shared runtime currently supports** (not the full Airbyte surface).
- [ ] Unit test over a couple of vendored manifests (one clean, one with an unsupported feature); workspace
  green + clippy clean.

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

### 2026-06-24 — DONE (commit 41c232c)
`weir_importer::analyze(yaml) -> ImportReport { tier(A/B/C/F), confidence, streams[], unsupported[] }` —
pure, no run/register. Classifies each stream vs the **shared runtime's current capability** (base_url/path/
record-selector/page-pagination/datetime-cursor = supported); names auth, offset pagination (degraded),
unsupported cursors, missing inline schema, unparseable input as gaps. `App::preview_manifest` wraps it;
`POST /catalog/preview {manifest}` returns it (re-exported `weir_app::ImportReport`). Test: clean → A/1.0;
bearer-auth → B + named gap; garbage → F. **45 groups / 59 tests green, clippy clean.**
