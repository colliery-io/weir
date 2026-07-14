---
id: embedded-dioxus-web-ui-served-by
level: task
title: "Embedded Dioxus web UI served by weir"
short_code: "WEIR-T-0017"
created_at: 2026-06-19T02:58:46.054232+00:00
updated_at: 2026-06-19T03:08:20.143763+00:00
parent: WEIR-I-0002
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0002
---

# Embedded Dioxus web UI served by weir

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0002]]

## Objective **[REQUIRED]**

A **Dioxus web UI** (Rust → WASM, running in the browser) for configure + monitor ([[WEIR-S-0003]]), **embedded in the `weir` binary** and served by the control-plane API ([[WEIR-T-0016]]) — single self-contained binary, no separate JS toolchain at runtime.

### Approach
- `weir-ui` standalone crate (own workspace, like the wasm fixtures): Dioxus 0.6 web app targeting `wasm32-unknown-unknown`, built with **trunk** → `dist/`.
- The app calls the JSON API (`/connections`, `.../run`, `.../runs`) via `gloo-net`: list connections, create, run, view run history.
- `weir-cli` embeds `dist/` (`include_dir!`) and the axum server serves it at `/` (assets + SPA fallback), alongside the JSON API.
- `cargo build` of the workspace must not require trunk (assets are prebuilt + embedded; a small fallback if absent).

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

- [x] `weir-ui` Dioxus 0.6 app compiles to `wasm32-unknown-unknown` and builds to `dist/` via trunk (632 KB wasm).
- [x] UI lists connections, has an add-connection form, a Run button, and a run-history panel — all over the JSON API (`gloo-net`).
- [x] `weir api` serves the embedded UI at `/` + assets + SPA fallback, alongside the JSON API; `cargo build` needs no trunk (build.rs embeds `dist/` if present, else a placeholder).
- [x] Tests: `GET /` → 200 HTML (oneshot); binary smoke-tested (`GET / → 200 text/html`, `/connections → []`).

## Status Updates

- **2026-06-19 — DONE.** `weir-ui` Dioxus 0.6 web app (standalone workspace) → WASM via **trunk** (`dx` unavailable; trunk bundles wasm-bindgen). `weir-cli/build.rs` embeds `weir-ui/dist` via generated `include_bytes!` (`UI_FILES`), empty→placeholder so the workspace builds without trunk; axum `fallback` serves the UI. **Artifact NOT committed** (`weir-ui/dist` gitignored, per direction) — a local `trunk build` produces the embedded UI; CI/fresh builds serve the placeholder. Real binary smoke-tested. **Slice 6 complete → all 6 initiative slices delivered.**

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
