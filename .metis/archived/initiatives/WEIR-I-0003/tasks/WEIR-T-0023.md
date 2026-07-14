---
id: wasm-connector-outbound-http
level: task
title: "WASM connector outbound HTTP (consume fidius wasi:http grant)"
short_code: "WEIR-T-0023"
created_at: 2026-06-19T13:46:25.497017+00:00
updated_at: 2026-06-21T02:17:13.218572+00:00
parent: WEIR-I-0005
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0005
---

# WASM connector outbound HTTP (consume fidius wasi:http grant)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0005]] (pulled from backlog 2026-06-19 — fidius 0.4 shipped `wasi:http`).

## Objective **[REQUIRED]**

**UNBLOCKED by fidius 0.4** (`wasi:http` egress + the 0.4.2 `PluginHost::builder().egress()` loader hook).

When unblocked: wire the granted outbound-HTTP capability into weir's WASM load path (the `caps` allow-list in `ConnectorHandle::from_wasm_package*`), and make the codegen'd WASM guest's `read` perform **real HTTP via `wasi:http`** instead of the current stub (`weir-codegen/src/wasm.rs`). A manifest REST connector then runs **live over the WASM (sandboxed) primitive**, not just the dylib — closing the open/community connector path. Credential injection rides the same host-import surface ([[WEIR-A-0013]]).

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

- [ ] (Blocked on fidius FR) WASM load path grants a capability-gated outbound-HTTP allow-list to the guest.
- [ ] `weir-codegen/src/wasm.rs` emits a real `wasi:http` `read` (replacing the stub); `rest` WASM regenerated and fetches live.
- [ ] E2E: a manifest REST source runs over the WASM primitive against a mock server and matches the dylib path (extends `tests/wasm_seam` / `rest`).

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

**2026-06-21 — fully scoped on fidius 0.5; one architecture fork to resolve.**
- **Guest HTTP** (`fidius_guest::http`): `get(url) -> Result<Response, HttpError>`; `Request::get(url).header(k,v).timeout(d)` + `send`; `Response { status, headers, body }`, `.text()`. `#[plugin_impl]` + `fidius_guest::http` auto-compose the `wasi:http` import (fixture `macro-fetcher`).
- **Read port** (from `weir-codegen/src/dylib.rs` `read_fn`): same paginated loop (path/pagination/cursor params → fetch → JSON parse → `record_selector` → field projection → cursor tracking), swapping `ureq` → `fidius_guest::http::get(&url)?.text()`. **Auth NOT set in the guest** — host `EgressPolicy` injects credentials ([[WEIR-A-0013]]); the guest just GETs (no `env:` cap).
- **Raw envelope**: the WASM guest must `decode`/`encode` the bincode `ReadContext`→`ReadOutcome` like the echo fixture → add a `weir-connector-types` dep to the codegen'd Cargo.toml (wasm-buildable).
- **Codegen** (`weir-codegen/src/wasm.rs`): pins 0.3.0→0.5.0; dep `weir-connector-types`; emit real `read` + `read_<stream>`; declare `http` capability. Regenerate `wasm-fixtures/rest`.
- **Runtime** (`weir-runtime`): `from_wasm_package_with_egress(..)` = `PluginHost::builder().egress(policy).build()` + `load_wasm`; plus a weir `EgressPolicy` (allow-list + cred injection).
- **E2E**: build `rest` wasm, load with egress, fetch a one-shot mock, assert records (extends `tests/wasm_seam`).

**⚠ FORK (resolved → A):** facade re-export, shipped in fidius **0.5.1**.

**2026-06-21 — Stage A DONE (committed `fb93265`), on fidius 0.5.1.**
- **Runtime egress seam ✅:** `ConnectorHandle::from_wasm_package_with_egress(search_path, package, policy)` (`PluginHost::builder().egress(policy).build()` + `load_wasm`) + `HostAllowList` `EgressPolicy` (allow-list + credential-header injection, [[WEIR-A-0013]]). Compiles clean.
- **⚠ fidius 0.5.1 facade-gating bug (for fidius):** the re-export is `#[cfg(feature = "host")] pub use fidius_host::executor::{EgressPolicy, EgressDenied};`, but those types live behind fidius-host's **`wasm`** feature (`executor::wasm`) — so via the `host`-only path they're "configured out." weir-runtime works around it by importing from `fidius_host::executor` directly (it already deps `fidius-host[wasm]` for `load_wasm`). Fix on fidius: gate the re-export on the wasm-enabling feature (or have `host` pull `fidius-host/wasm`).

**2026-06-21 — DONE (B/C/D complete; verified end-to-end).**
- **B. Codegen ✅** (`6c46ef1`/`d20caa0`): `weir-codegen/src/wasm.rs` emits a real `read` — decodes the `weir-connector-types` raw envelope (fully-qualified to dodge the WitType `Config`/`ConnectorError` collision), dispatches to per-stream `read_<stream>` helpers that paginate + cursor-track over `fidius_guest::http::get(&url)?.text()` (no in-guest auth; host injects). Emitted Cargo gains `weir-connector-types` + `serde_json`, pins 0.5.1.
- **C. ✅** Regenerated `wasm-fixtures/rest`; builds for wasm32-wasip2 (188KB artifact) — `#[plugin_impl]` + `fidius_guest::http` compose the `wasi:http` import.
- **Resolved OPEN:** the `[wasm].capabilities = ["http"]` is written at **staging** (the test's package.toml), not by codegen — matches `wasm_seam`'s pattern.
- **D. E2E ✅** (`tests/wasm_http.rs`): allow → the connector fetches the record live over `wasi:http` + tracks the cursor; deny → connector error. **56 workspace groups green, clippy clean.**

✅ **T-0023 complete** — codegen'd WASM connectors run live HTTP over the sandboxed primitive, EgressPolicy-brokered (credential injection + allow-list). Remaining nit is fidius's (the facade re-export feature-gating).
