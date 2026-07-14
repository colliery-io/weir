---
id: codegen-v1-emit-streaming-read
level: task
title: "Codegen v1: emit streaming read + configured construction"
short_code: "WEIR-T-0032"
created_at: 2026-06-21T03:25:52.248298+00:00
updated_at: 2026-06-21T19:31:17.710076+00:00
parent: WEIR-I-0006
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0006
---

# Codegen v1: emit streaming read + configured construction

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0006]]

## Objective **[REQUIRED]**

Slice 4 of [[WEIR-I-0006]] (per [[WEIR-A-0029]]): `weir-codegen` emits v1 — a streaming `read` returning `fidius_guest::Stream<ReadMessage>` (drop the `has_more` loop) over the per-stream HTTP/cursor logic, and configured construction (`configure(Config)`), for both dylib + wasm backends. Depends on [[WEIR-T-0029]].

**AC:** `weir-codegen` generates v1 connectors; `rest` (dylib + wasm) regenerated, builds, and streams; `wasm_http`/`rest` E2E updated + green.

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

**2026-06-21 — IN PROGRESS. Connector v1 pattern found** (fidius fixture `macro-configured-stream`): `struct X { cfg }` + `#[plugin_impl(Connector, crate="weir_connector::fidius", config = Config)]` + `fn read(&self, ctx) -> weir_connector::fidius::Stream<ReadMessage> { Stream::from_iter(...) }` + `impl X { fn configure(cfg: Config) -> Self }`. **Contract refinement (amends [[WEIR-T-0029]]):** add `ReadMessage::Fatal(ConnectorError)` so a from_iter stream can signal a fatal read error (engine maps it → `EngineError::Connector`); requires `ConnectorError: WitType`. **Codegen plan:** dylib + wasm `read` → fetch via existing `read_<stream>` (eager v0; lazy-per-page is a later optimization) then `Stream::from_iter([Records(Rows), Checkpoint{cursor}])`, or `[Fatal(e)]` on error; `base_url` from `self.cfg` (bound) not `ctx.config`; emit `struct {cfg}` + `config = Config` + `configure`. Regenerate `rest` (dylib+wasm), build. **This same pattern drives [[WEIR-T-0033]]** (hand-written echo/faulty/arrow-sink/postgres).

**2026-06-21 — dylib codegen DONE** (committed `626a3e8`): `weir-connector-rest` regenerated to v1 (configured streaming) + builds warning-free. **wasm codegen remaining — KEY CONSTRAINT:** the WASM guest's `read` is now a **typed** streaming method (`ReadContext` arg + `Stream<ReadMessage>`), so those types must appear in the guest's WIT. `fidius_build::emit_wit()` parses the **guest's own `src/lib.rs`** for `#[derive(WitType)]` defs — so the read-path types must be **defined locally** in the guest's `weir_guest_types` module and **exactly match the host contract's WIT** (field order/names/variants) or the load-time interface-hash check fails. Add to `wasm.rs`'s `weir_guest_types`: `RecordBatch`, `ArrowIpc`, `StreamState`, `LogLevel`, `LogEntry`, `DeadLetter`, `Partition`, `ConfiguredStream`, `WriteMode`, `MappingSpec`, `MappingOp`, `ReadContext`, `ReadMessage` (all WitType, mirroring `weir-connector-types`). Then: interface `read -> fidius_guest::Stream<ReadMessage>`; `struct {cfg}` + `#[plugin_impl(config = Config)]` + `configure`; impl read fetches (existing `read_<stream>` over `fidius_guest::http`) then `fidius_guest::Stream::from_iter([Records, Checkpoint] | [Fatal])`; `base_url` from `self.cfg`. Rebuild `wasm-fixtures/rest` wasm32 + verify hash (the `wasm_http` E2E, updated in [[WEIR-T-0033]]).
