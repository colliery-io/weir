---
id: contract-v1-types-streaming-read
level: task
title: "Contract v1 types: streaming read (ReadMessage) + configured-instance lifecycle"
short_code: "WEIR-T-0029"
created_at: 2026-06-21T03:24:14.522521+00:00
updated_at: 2026-06-21T03:39:48.017610+00:00
parent: WEIR-I-0006
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0006
---

# Contract v1 types: streaming read (ReadMessage) + configured-instance lifecycle

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0006]]

## Objective **[REQUIRED]**

Slice 1 of [[WEIR-I-0006]] (per ratified [[WEIR-A-0029]]) — the **foundational contract change** in `weir-connector-types`/`weir-connector`:
- streaming shape `read(ReadContext) -> Stream<ReadMessage>`, `ReadMessage = Records | Checkpoint | Log | DeadLetter` (replaces chunked-pull `ReadOutcome{has_more}`);
- configured-instance lifecycle: typed `Config` bound at construction; `ReadContext`/`WriteContext` drop per-call `config`;
- bump contract version v0 → v1.

Types-only; consumers (engine/runtime/codegen/connectors) follow in [[WEIR-T-0030]]–[[WEIR-T-0034]]. Gate on `cargo build -p weir-connector-types -p weir-connector` (the full workspace won't build until the consumer slices land).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] `ReadMessage` + streaming `read` in the contract; `Config` off the contexts; contract version = 1; aligns with fidius 0.5 `Stream<T>` + configured instances.
- [ ] `weir-connector-types` + `weir-connector` compile.

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

**2026-06-21 — DONE (contract crates build).** `weir-connector-types`: added `ReadMessage = Records | Checkpoint | Log | DeadLettered` (WitType) + WitType cascade on `RecordBatch`/`ArrowIpc`/`StreamState`/`LogEntry`/`LogLevel`/`DeadLetter`/`Partition`/`WriteMode`/`MappingSpec`/`MappingOp`/`ConfiguredStream`/`ReadContext`/`WriteContext`; dropped `config` from `ReadContext`+`WriteContext` (configured instances). `weir-connector`: trait `read` → `fn read(&self, ctx: ReadContext) -> crate::fidius::Stream<ReadMessage>` (removed `#[wire(raw)]`; `#[plugin_interface]` accepts the `Stream` return). **Gate met:** `cargo build -p weir-connector-types -p weir-connector` (default + `wit`) + clippy clean. **Wire-encoding decision:** `ReadMessage` crosses as a typed (WitType) stream item; Arrow rides as `list<u8>` in `RecordBatch::Arrow` (JSON `Rows` is the common path) — efficiency follow-up if Arrow streaming gets hot. **Workspace red by design** (consumers reference `ctx.config`/old `read`/`ReadOutcome`) — resolved by [[WEIR-T-0030]] (runtime), [[WEIR-T-0031]] (engine), [[WEIR-T-0033]] (connectors).
