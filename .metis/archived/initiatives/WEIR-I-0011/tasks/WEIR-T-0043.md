---
id: s3-flip-retire-the-native-path
level: task
title: "S3: Flip + retire the native path"
short_code: "WEIR-T-0043"
created_at: 2026-06-22T20:03:31.458815+00:00
updated_at: 2026-06-23T02:57:45.058080+00:00
parent: WEIR-I-0011
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0011
---

# S3: Flip + retire the native path

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0011]]

## Objective **[REQUIRED]**

S3 of [[WEIR-I-0011]] — the flip. Once the connectors run on wasm (S1+S2), **retire the native path**:
delete `weir-codegen/dylib.rs`, `ConnectorHandle::native_in_process`, and `ConnectorRef::Native`;
`ConnectorRef` becomes `{ package, version, origin }` ([[WEIR-A-0019]] version + [[WEIR-A-0030]] origin);
the engine/orchestrator/app/api resolve **wasm-only**. AC: no cdylib connector code path remains; the
**full 57-group suite is green on wasm**; clippy clean.

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

### 2026-06-22 — prerequisite met + key sequencing finding (S3 not yet started)
**Connector side of the flip is complete:** all five demo connectors have working wasm guests that build
for `wasm32-wasip2` — `echo` + `rest` (pre-existing) and `slow`/`faulty`/`arrow-sink` (S2). (postgres
is S4.) So there's a wasm guest for everything the suite/demo needs except postgres.

**Blast radius (scanned):** `ConnectorRef` (orchestrator; already has `Native` + `Wasm{search_path,
package}`) + `resolve`/`native_in_process` (orchestrator/runtime) + `native()`/`plugin_name`/
`connector_spec` (app) + cli + api callers + ~8 test files (e2e, engine, rest, sink, seam, wasm_*) +
`weir-codegen/dylib.rs` + the native connector crates.

**Key finding — S3 needs app-side wasm resolution first (intertwined with [[WEIR-I-0010]]):** retiring
`ConnectorRef::Native` breaks `App::native(name)` / api / cli, which today turn a *plugin name* into a
`Native` ref. To make them produce a **`Wasm` ref** they must know **where the package lives** (search_path
= a `connectors/` dir) + its **version/origin** — i.e. a **catalog lookup**. So the flip can't be a clean
"delete native"; it requires the **app to locate/stage wasm packages** (a `connectors/` dir populated by
building the guests — the local A-0018 compile + a minimal I-0010 catalog). Confirms I-0010 ⇄ I-0011-flip
are intertwined.

**Sequenced remaining plan:**
1. **App-side wasm resolution** — a `connectors/` dir; build+stage the guest packages (the `weir-wasm-testkit`
   seam at app/demo scope); `App::native(name)` → resolve to a `Wasm` ref from the dir (minimal catalog).
2. **Reshape `ConnectorRef`** → `Wasm { package, version, origin }` (retire `Native`); `resolve` wasm-only.
3. **Migrate tests** off `native_in_process` → `from_wasm_package` via the testkit.
4. **Retire** `native_in_process`, `weir-codegen/dylib.rs`, and the native connector crates; full suite +
   demo green on wasm; clippy clean.

⇒ Step 1 is really the **I-0010 catalog MVP**; recommend decomposing/advancing I-0010 (app-side wasm
resolution) before deleting native. Tree clean; all guest artifacts built.

### 2026-06-22 — DONE (all 4 steps; commits 7f22333, e0f1d54, bfa7ccd, f9af775)
1. **App-side wasm resolution** — `App::connector_ref(name)` resolves to a wasm package under
   `WEIR_CONNECTORS_DIR` (default `connectors/`); cli/api/spec use it; `angreal ui demo` builds+stages the
   guests. (The minimal I-0010 catalog MVP.)
2. **`ConnectorRef` wasm-only** — `Native` variant removed; `resolve` wasm-only; `plugin_name` normalizes
   `weir-<x>-pkg`→`x`. (Deferred `version`/`origin` fields on the ref to a follow-up — not needed to retire
   native.)
3. **Tests migrated** off `native_in_process` → `weir_wasm_testkit::{load, connectors_dir}` (compile-once
   cache): engine, orchestrator (`wasm_ref` helper), runtime, arrow-sink, app (cli/serve), api. Deleted
   redundant native-only tests (`rest.rs`→`wasm_http_engine`, `seam.rs`→`wasm_seam`); retired
   `transient_blip_recovers_across_ticks` (connector self-recovery via process-global state is incompatible
   with wasm isolation — the correct sandbox property).
4. **Native retired** — deleted `ConnectorHandle::native_in_process`, the 5 native connector crates
   (echo/slow/faulty/arrow-sink/rest), and the **dylib codegen** (`dylib.rs`, `generate_dylib_crate`,
   `auth_header_stmt`, the `dylib` codegen target + test). `angreal test connectors` + a compile-once build
   before `test all`.

**Verified live**: `angreal ui demo` runs entirely on wasm — connections resolve `slow`/`echo`/`faulty`/
`arrow-sink` to wasm guests; slow-stream `done`/12 rows; faulty dead-letters. **49 groups green; clippy
clean.** Only `postgres` native crate remains (S4 — its tests don't use in-process loading).

**Follow-up (not blocking):** add `version`/`origin` to `ConnectorRef::Wasm` (A-0019 pinning + A-0030
origin) — the data-model refinement, deferred from this flip.
