---
id: s2-declarative-import-fidelity
level: task
title: "S2: Declarative-import fidelity harness (curated MIT corpus + coverage report)"
short_code: "WEIR-T-0053"
created_at: 2026-06-24T09:42:32.923189+00:00
updated_at: 2026-06-24T09:42:32.923189+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/todo"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S2: Declarative-import fidelity harness (curated MIT corpus + coverage report)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0008]] — the **fidelity harness** ([[WEIR-A-0020]]) that runs `weir-importer` over a corpus
of Airbyte declarative `manifest.yaml`s and reports, per connector, **tier + confidence + pass/fail** plus
an aggregate coverage report — the gate every coverage slice (S3–S7) reports a pass-rate delta against, so
we **never silently emit a broken connector**. **First pass uses a small *curated* MIT/Apache/BSD set**
(hand-picked, vendored test-only under `corpus/` with upstream license + commit-SHA provenance + NOTICE);
the automated whole-catalog license filter + bulk vendor is a follow-on once the harness exists and corpus
fetch from `airbytehq/airbyte` is proven in this environment. Recorded-HTTP record-fidelity on a tiny subset.

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

## Acceptance Criteria **[REQUIRED]**

- [ ] A **curated `corpus/`** (test-only, excluded from shipped crates) of a handful of permissive
  (MIT/Apache/BSD) Airbyte declarative `manifest.yaml`s, each with **upstream license + commit-SHA
  provenance** + aggregate **NOTICE** attribution. (Confirm fetch from `airbytehq/airbyte` works; if blocked,
  vendor by hand from known sources.)
- [ ] A **harness** that runs `weir-importer` over every corpus manifest and emits per-connector **tier +
  confidence + pass/fail** + an **aggregate coverage report** (the live "parity %").
- [ ] **Never silently emit a broken connector**: unsupported declarative features are reported (tier/
  confidence), not dropped — the report names the gap.
- [ ] **Record-fidelity check** on ≥1 connector via recorded HTTP (assert weir emits the expected records).
- [ ] Runs in the suite (gated/`#[ignore]` as appropriate); the report is reproducible; provenance/NOTICE
  present. Hook for the future automated license-filter is stubbed/noted. Workspace green + clippy clean.

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

### 2026-06-30 — closed as superseded (not built)

Closed without building the "harness." The actual need — **tracking which connector constructs are
supported and what's left to implement** — is served by **[[WEIR-S-0016]]** (the declarative parity
ledger). Maintaining that spec doc (flipping rows as constructs land, listing each ❌ + why) *is* the
tracking process; a separate corpus/coverage "harness" was a tool invented for a job the spec already
does. The honest-coverage discipline this task wanted (never claim a construct we can't run) lives in
`weir-importer::analyze()` + the per-construct wire tests, which the ledger references.

If a concrete future need appears — a contributor CI gate, or a headline "imports N% of the real Airbyte
catalog" number — file a *new, specifically-scoped* task for that one thing (a coverage scan over a
license-filtered corpus, or record-diff fidelity on specific connectors), rather than a catch-all harness.
The ACs below are intentionally left unchecked (the harness was not built).
