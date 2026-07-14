---
id: fix-ci-workflows-green-kept
level: task
title: "Fix CI workflows (green, kept disabled while private)"
short_code: "WEIR-T-0022"
created_at: 2026-06-19T13:46:19.918462+00:00
updated_at: 2026-06-19T16:33:21.079201+00:00
parent: WEIR-I-0003
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0003
---

# Fix CI workflows (green, kept disabled while private)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0003]]

## Objective **[REQUIRED]**

The GitHub Actions workflows (`ci.yml`, `docs.yml`, `release.yml`) were red on `main` before/through Phase 1 and are now disabled repo-wide (private-repo cost). Fix the workflow definitions so they pass — fmt + clippy + `cargo test` across the workspace, correctly handling the standalone `weir-ui` workspace (separate `[workspace]`, wasm target) and any docs build. Keep Actions **disabled** while the repo is private, but leave the workflows correct so they can be re-enabled cleanly on going public ([[WEIR-V-0001]] ASF path).

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

- [x] `ci.yml` rewritten: fmt + clippy(`-D warnings`) + `cargo test --workspace` + release build (ubuntu/macos). `weir-ui` is its own workspace (root excludes it) so no wasm toolchain needed. Dropped the template's nonexistent `--test integration`/`--test functional` jobs + the coverage job.
- [x] `docs.yml` pared to `workflow_dispatch` (manual — deploys to Pages + needs `plissken`); `release.yml` unchanged (tag-gated). Dead `weir-core` template crate removed.
- [x] Validated locally — fmt/clippy/`test --workspace`/`build --release` all green (fixed clippy lints + `profile.release` `lto=false` for the rlib+cdylib connectors). Actions remain disabled repo-wide; re-enable via `gh api -X PUT repos/colliery-io/weir/actions/permissions -F enabled=true` on going public.

## Status Updates

- **2026-06-19 — DONE.** The red was the angreal template's CI referencing `--test integration`/`--test functional` (only existed in the dead `weir-core` placeholder) + `clippy -D warnings` failing. Rewrote `ci.yml` to the real pipeline, made clippy clean, removed `weir-core`, set `lto=false` (LTO can't load the cdylib connectors' bitcode → release build failed). docs → manual. All four CI commands validated locally; no billable runs (Actions stay disabled).

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
