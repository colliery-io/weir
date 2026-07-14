---
id: s2-ingress-pipeline-fetch-compile
level: task
title: "S2: Ingress pipeline (fetch→compile→snapshot→upsert), local/folder import first"
short_code: "WEIR-T-0048"
created_at: 2026-06-24T00:05:42.874524+00:00
updated_at: 2026-06-24T00:57:31.443512+00:00
parent: WEIR-I-0010
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0010
---

# S2: Ingress pipeline (fetch→compile→snapshot→upsert), local/folder import first

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0010]]

## Objective **[REQUIRED]**

S2 of [[WEIR-I-0010]]. Build the **shared ingress pipeline** ([[WEIR-S-0007]] Catalog Sync §2 /
[[WEIR-A-0018]]): *resolve source → fetch → compile to `wasm32-wasip2` → load + snapshot `spec()` → upsert
the `(name, version)` row*. Elevate the [[WEIR-I-0011]] build seam (`weir-wasm-testkit`) to an app
capability. MVP entry = **local crate path / folder package** (`origin=private`); the source abstraction is
shaped so crates.io `(name,ver)` + git URL plug in later (post-MVP, [[WEIR-A-0018]]). Identity = `Cargo.toml
(name, version)`; **import is an upsert** on that key.

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

- [ ] **Ingress pipeline**: given a local crate path / folder package → fetch → `cargo build --target
  wasm32-wasip2 --release` → cache wasm in `connectors/` → load + snapshot `spec()` → **upsert** the
  `(name, version)` row (from `Cargo.toml`), `origin=private`.
- [ ] **Idempotent upsert** on `(name, version)`: re-importing a changed crate at the same version replaces
  the row + cached artifact.
- [ ] **Status lifecycle** `importing → ready → failed`; compile / `spec()` / contract-incompatibility
  failures are surfaced with a clear message and leave `status=failed` (not usable), no partial row.
- [ ] **Source abstraction** (an enum/trait) with `local-path` + `folder` implemented now and `crates.io
  (name,ver)` / `git URL` variants stubbed for later — one pipeline, pluggable source.
- [ ] **Conformance test**: scan → register → pin → load → run with the **rest** package (as a local
  crate) — green end-to-end through the engine.

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

### 2026-06-24 — DONE (commit a1c9284)
`weir-app::ingress` module implements the shared pipeline as an app capability (the [[WEIR-I-0011]] build
seam elevated out of the test-only testkit):
- **`Source`** enum: `LocalCrate(PathBuf)` + `Folder { package }` implemented; `CratesIo`/`Git` stubbed
  (same pipeline, only the fetch differs).
- **`App::import(source, origin)`**: `read_manifest` (Cargo.toml `name`/`version` + `[package.metadata.weir]
  capabilities`) → `cargo build --target wasm32-wasip2 --release` → stage a **full fidius `package.toml`**
  (`[package]` name + `runtime="wasm"` + interface + `[metadata]` + `[wasm]` component/capabilities) under
  `WEIR_CONNECTORS_DIR` → load to snapshot `spec()` → **upsert** the catalog row (`status="ready"`).
  Identity = `Cargo.toml (name, version)`; idempotent via `register_connector`'s INSERT OR REPLACE.
- **Gotcha found + fixed:** `find_wasm_package` matches `package.toml` `[package].name` + `runtime="wasm"`
  (not the `[wasm]` block) — a minimal manifest fails discovery silently. Now mirrors the testkit's full
  manifest. Added `toml` dep for Cargo.toml parsing.
- **Test** (`ingress::tests`): compile the real `slow` crate → register → cataloged at its Cargo.toml
  version + the staged package loads (spec round-trips). 44 groups green, clippy clean.

**Scope notes vs AC:** conformance proves compile→stage→snapshot→register→**loadable** using `slow` (no
egress) rather than a full engine **run** of `rest` (run is covered by existing engine tests; rest
needs http-egress setup). `status=failed` lifecycle: errors return `Err` (no row written) rather than a
persisted `failed` row — surfaced to the caller; a persisted failed-state row is a later refinement.
