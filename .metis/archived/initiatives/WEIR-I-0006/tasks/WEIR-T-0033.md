---
id: port-connectors-to-v1-conformance
level: task
title: "Port connectors to v1 + conformance (echo/faulty/arrow-sink/postgres, CDC as stream)"
short_code: "WEIR-T-0033"
created_at: 2026-06-21T03:25:54.642775+00:00
updated_at: 2026-06-21T19:31:04.237878+00:00
parent: WEIR-I-0006
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0006
---

# Port connectors to v1 + conformance (echo/faulty/arrow-sink/postgres, CDC as stream)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0006]]

## Objective **[REQUIRED]**

Slice 5 of [[WEIR-I-0006]] (per [[WEIR-A-0029]]): port the hand-written connectors to v1 — `weir-connector-echo`, `weir-connector-faulty`, `weir-connector-arrow-sink`, and `weir-connector-postgres` (incl. **CDC re-expressed as a live change stream**, collapsing the [[WEIR-T-0028]] `get_changes` polling). Partitioned reads stay one configured stream per `Partition` ([[WEIR-T-0027]]). Re-run conformance (incl. live Postgres). Depends on [[WEIR-T-0029]]–[[WEIR-T-0031]].

**AC:** all connectors compile + pass conformance on v1; partitions + CDC re-expressed over the stream; workspace + integration green.

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

### 2026-06-21 — connectors ported (v1), non-test workspace green, rename reverted
- **All 5 dylib connectors ported to v1** (commits up to e69f673): echo, faulty, arrow-sink,
  postgres, slow → `struct{cfg}` + `#[plugin_impl(config = Config)]` + `read -> Stream<ReadMessage>`
  via `Stream::from_iter` + inherent `configure(cfg)`. Postgres pure fns (`read_records`/`cdc_read`/
  `write_records`) unchanged; CDC re-expressed as `Records`+`Checkpoint` over the stream. weir-app
  `work_spec` updated. **Whole non-test workspace + bins compile + clippy-clean on v1.**
- **Engine v1 verified end-to-end:** 16 orchestrator tests pass (engine `block_on` async read inside
  `spawn_blocking` + configured dylib connectors + relay/worker, incl. partitioned fan-out + retry/
  backfill). The runtime-nested-runtime concern from [[WEIR-T-0031]] is resolved (no panic).
- **`ArrowIpc` tuple->named fields** (emit_wit rejects tuple structs) — kept (separate from keywords).
- **Reserved-WIT-keyword field rename BACKED OUT** (e69f673): `ConfiguredStream.stream` is natural
  again. Decision: fix in fidius (`emit_wit` escapes reserved keywords, matching the `WitType`
  derive) rather than rename a growing class of contract fields (`stream` x2, `record`, `from`).
  Request written; **user is publishing the fidius fix.**

### 2026-06-21 (later) — full non-wasm suite green; ReadOutcome removed; wasm tests compile
- All v1-port test fixes landed (commits up to b41ae2a): engine/e2e/rest/sink/seam + the
  weir-connector round-trip + codegen shape tests. **Whole workspace lib + non-wasm integration
  tests pass on v1** (16 orchestrator, engine, rest, e2e-native, seam, arrow-sink).
- Removed dead v0 `ReadOutcome` (fully replaced by the `ReadMessage` stream); round-trip test now
  exercises every `ReadMessage` variant; module docs updated to v1.
- wasm runtime tests (wasm_seam/wasm_http/wasm_signing) **compile** on v1 (tokio dev-dep, async
  read stream, base_url moved into construction Config) but need fidius 0.5.3 fixtures to RUN.
- **fidius 0.5.3 NOT yet on crates.io** (`cargo` can't select `^0.5.3` after index refresh) —
  the wasm path (WEIR-T-0032 + these 3 tests) stays gated until it publishes.

### Remaining (post-fidius)
- Finish the v1-port test fixes: `weir-engine/tests` (engine/e2e/rest — `native_in_process(name,&cfg)`,
  `sync` dropped the config param, `read` is async->`Stream`), `arrow-sink/tests/sink.rs`,
  and the runtime `tests` (seam/wasm_seam/wasm_http/wasm_signing — `from_wasm_package*(…, &cfg)`,
  `read().await` consuming the stream).
- **WEIR-T-0032 wasm codegen** (blocked): once fidius escapes keywords, the wasm codegen already
  written in `crates/weir-codegen/src/wasm.rs` should generate a building guest — regen
  `wasm-fixtures/{echo,rest}`, `cargo build --target wasm32-wasip2`, then wasm conformance.
- Re-run live Postgres integration conformance.
