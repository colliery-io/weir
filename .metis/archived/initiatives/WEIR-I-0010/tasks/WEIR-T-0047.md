---
id: s1-versioned-catalog-store-per
level: task
title: "S1: Versioned catalog store + per-connection pinning"
short_code: "WEIR-T-0047"
created_at: 2026-06-24T00:05:35.972780+00:00
updated_at: 2026-06-24T00:36:22.904981+00:00
parent: WEIR-I-0010
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0010
---

# S1: Versioned catalog store + per-connection pinning

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0010]]

## Objective **[REQUIRED]**

S1 of [[WEIR-I-0010]]. Stand up the persisted **`connectors` catalog table** keyed **`(name, version)`** in
the diesel-dualdb `Store`; give **`ConnectorRef::Wasm` a `version` (+ `origin`)** and backfill existing
connection rows; expose a **registry read API** over the table; and **gate dispatch** on the pinned
version's `contract_range`. Subsumes the deferred [[WEIR-I-0011]] ConnectorRef-versioning follow-up. Data
model + pinning only — the ingress pipeline that *fills* the table is S2 ([[WEIR-T-0048]]).

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

- [ ] `connectors` table in `Store` — **PK `(name, version)`** (semver); columns: `roles`,
  `config_schema`, `contract_range`, `supported_sync_modes`, `origin` (`first-party|community|private`),
  `status`, `location`/artifact-path, timestamps. Migration on both SQLite + Postgres (diesel-dualdb).
- [ ] `ConnectorRef::Wasm` gains **`version`** (+ `origin`); existing connection rows **backfilled** on
  migration; `resolve` unchanged (still `from_wasm_package`).
- [ ] Registry **read API** over the table (list all; get by `(name, version)`); enumeration is a **table
  read**, no connector load.
- [ ] **Dispatch gate**: a run resolves the connection's pinned `(name, version)`, checks its
  `contract_range` against the running engine contract; incompatible → refused with a clear error (no run).
- [ ] Unit tests: persist → reopen → still enumerable; a **pinned version survives a newer registration**
  (no auto-upgrade, per [[WEIR-A-0019]]).

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

### 2026-06-24 — started; orientation + plan
**Pattern (weir-app):** inline `CREATE TABLE IF NOT EXISTS` in `App::open`; raw diesel `sql_query` +
`#[derive(QueryableByName)]` rows; `ConnectorRef` serialized to `source_ref`/`dest_ref` via `json()`.
`ConnectorSpec.contract_version: u32`. `ConnectorRef::Wasm { search_path, package }` (orchestrator);
`resolve()`→`from_wasm_package`. Build sites: `weir-app::connector_ref`, `tests/common::wasm_ref`;
destructure in `resolve` + `plugin_name` (already `..`).

**Plan:** (1) `ConnectorRef::Wasm` gains `version: String` + `origin: Origin` (first-party|community|private),
`#[serde(default)]` so old JSON backfills; fix sites; `resolve` unchanged. (2) `connectors` table (PK
`(name,version)`) + `CatalogEntry` + register/list/get. (3) dispatch gate on pinned `contract_version` vs
engine. (4) unit tests (persist→reopen; pin survives newer registration).

### 2026-06-24 — DONE (commits e6271f5, 32cdb63)
- **`ConnectorRef::Wasm` + `Origin`** (orchestrator): added `version: String` + `origin: Origin`
  (`FirstParty|Community|Private`), both `#[serde(default)]` → old `source_ref`/`dest_ref` JSON backfills
  (`0.0.0` / `Private`). `resolve()` unchanged (`..`). Sites fixed (`connector_ref`, `wasm_ref`). **Clears
  the deferred [[WEIR-I-0011]] ConnectorRef-versioning debt.**
- **`connectors` table** (weir-app `App::open`): PK `(name, version)` + `roles`, `config_schema`,
  `contract_version`, `supported_sync_modes`, `origin`, `status`, `location`, timestamps.
- **Read API**: `CatalogEntry` + `register_connector` (INSERT OR REPLACE upsert) / `list_connectors` /
  `get_connector(name, version)` — table reads, no connector load. `Origin` re-exported.
- **Dispatch gate** (`plan_run` → `check_contract`): a cataloged pin whose `contract_version` ≠
  `ENGINE_CONTRACT_VERSION` (=1) is refused with `AppError::Contract`; **uncataloged refs pass**
  (gate-if-present → non-breaking; engages as the catalog fills).
- **Tests** (`catalog_tests`): persist→reopen→enumerable; pin survives a newer registration; gate
  refuses incompatible then allows compatible. 44 groups green, clippy clean.

**Note:** `register_connector` is the upsert seam S2 ([[WEIR-T-0048]]) calls after compile + spec snapshot.
AC line said "contract_range" — implemented as a single `contract_version` gate (range widening is future).
