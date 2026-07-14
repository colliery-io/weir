---
id: integration-test-harness-angreal
level: task
title: "Integration test harness: angreal + docker-compose Postgres"
short_code: "WEIR-T-0024"
created_at: 2026-06-19T17:50:22.709605+00:00
updated_at: 2026-06-19T17:56:49.685904+00:00
parent: WEIR-I-0004
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0004
---

# Integration test harness: angreal + docker-compose Postgres

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0004]]

## Objective **[REQUIRED]**

Slice 0 of [[WEIR-I-0004]]: stand up the **integration-test harness** that later slices use to validate the contract against a real database. A `docker-compose.yml` Postgres service + an angreal `integration` task group (`up`/`down`/`status`/`test`) to manage it, plus a smoke integration test (gated `#[ignore]`, pure-Rust `postgres` client — no libpq) proving Rust can connect to the containerized Postgres. No engine changes yet — this is the foundation for write modes / incremental / partitioned / CDC.

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

- [x] `docker-compose.yml` — Postgres 16 service with a `pg_isready` healthcheck.
- [x] `.angreal/task_integration.py` — `integration` group: `up` (`docker compose up -d --wait`), `down` (`-v`), `status`, `test`. `angreal tree` lists them.
- [x] `weir-connector-postgres` (slice-0 stub) + `#[ignore]` smoke test (pure-Rust `postgres`) — connects + `SELECT 1`. **Proven**: `up` → test passes → `down`.
- [x] `.github/workflows/integration.yml` — Postgres service-container job (new file, sidesteps the held `ci.yml` rewrite; Actions stay disabled). Command validated locally.

## Status Updates

- **2026-06-19 — DONE.** Branch `feat/weir-i-0004-full-contract` (commit `88a8740`). docker-compose Postgres + angreal `integration` tasks + `weir-connector-postgres` stub with an ignored connectivity smoke test (rust-postgres). Verified `up → ignored test green → down`; normal `cargo test --workspace` skips it (53 groups green). Branched from `main` (slice 0 is new files, independent of the held hardening code).

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
