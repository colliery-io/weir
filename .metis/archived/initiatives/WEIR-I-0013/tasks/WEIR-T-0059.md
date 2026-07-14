---
id: s1-foundation-weir-engine-codegen
level: task
title: "S1: Foundation + weir-engine — codegen harness, dispatched migration runner, portable Store"
short_code: "WEIR-T-0059"
created_at: 2026-06-25T03:09:13.250186+00:00
updated_at: 2026-06-25T11:14:38.061548+00:00
parent: WEIR-I-0013
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0013
---

# S1: Foundation + weir-engine — codegen harness, dispatched migration runner, portable Store

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0013]]

## Objective **[REQUIRED]**

S1 of [[WEIR-I-0013]] — stand up the diesel-dualdb portable-store pattern end-to-end and
prove it on the smallest surface, weir-engine's `Store`. Author the logical-DDL migration
for the engine tables, wire the `diesel-dualdb-schema` codegen (generated per-backend
migrations + `schema.rs`), replace `Store::migrate()`'s raw SQLite DDL with a
`DualConnection::dispatch` migration runner, and convert the 13 engine `sql_query` sites
to the typed query builder + portable `sql_types`. De-risks the whole initiative: codegen,
runner, typed builder, and a dual-backend test all exercised once before the larger
surfaces ([[WEIR-T-0060]], [[WEIR-T-0061]]) adopt the pattern.

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

- [ ] A logical-DDL migration covers `dead_letters`, `stream_state` (`opaque` as `BYTES`),
  `outbox`, `run_logs`, with portable autoincrement for the rowid `id`s.
- [ ] `diesel-dualdb-schema` generates per-backend migrations + a portable `schema.rs`
  (`table!` over `diesel_dualdb::sql_types`); generated output committed; an `angreal`
  task regenerates it reproducibly.
- [ ] `Store::migrate()` runs the generated migration via `DualConnection::dispatch`
  (Pg vs Sqlite arm) — no raw dialect DDL remains in weir-engine.
- [ ] All 13 weir-engine `sql_query` sites use the typed query builder against
  `&mut DualConnection`; any `dispatch` is documented as genuinely backend-divergent.
- [ ] weir-engine comes up + round-trips on Postgres (`angreal integration` /
  `DUALDB_PG_URL`) AND the existing SQLite suite stays green; ≥1
  `#[diesel_dualdb::test(pg, sqlite)]` proves the pattern.

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

### 2026-06-25 — Foundation DONE (commit 4587db1); engine conversion remains

**Done (compiles clean):**
- New **`crates/weir-schema`** crate (shared, bottom of the dep graph for engine/app/orch).
- Logical migration `schema/migrations/0001_init/up.sql` — engine tables. **Key decision
  (user-approved "we can change the keys"):** surrogate autoincrement `id`s → **client-gen
  UUID PKs** (the diesel-dualdb idiom; the `diesel-dualdb-schema` CLI has **no SERIAL/
  autoincrement** — `Integer→INTEGER` both backends). Recency that used `ORDER BY id DESC`
  → `ORDER BY ts DESC` (`run_logs` already has `ts`; **added `ts BIGINT` to `dead_letters`**).
  `stream_state.opaque BLOB → BYTEA` (logical), composite PK `(connection, stream)`.
- Codegen verified: `diesel-dualdb-schema` → `schema/generated/{schema.rs, migrations-
  postgres, migrations-sqlite}`. Deterministic (re-run = no diff). Wired **`angreal schema
  gen`** (prefers installed binary, falls back to local `../diesel-dualdb` cli).
- `weir_schema::migrate(&mut DualConnection)` — idempotent, **dispatches the per-backend
  `up.sql` by `DualConnection::Pg|Sqlite` arm**, guarded by a portable `__weir_schema_version`
  sentinel (`CREATE TABLE IF NOT EXISTS` + parameterless count — both-backend safe).

**Remaining for S1 (the larger half — iterative diesel work):**
1. weir-engine deps: add `weir-schema`, `uuid`, enable diesel-dualdb `uuid` feature.
2. `Store::migrate()` → call `weir_schema::migrate`; delete the raw `CREATE TABLE` DDL +
   the `Store::open` SQLite-only note (lib.rs:9-10).
3. Convert the **13 `sql_query` sites** to the typed builder over `weir_schema::{dead_letters,
   stream_state, outbox, run_logs}`; the run-loop transactional inserts (lib.rs ~433/455/482/
   500) now set `uuid::Uuid::new_v4()` `id` (+ `ts = now_ms()` for dead_letters). The
   stream_state upsert is the likely `DualConnection::dispatch` spot (on_conflict differs).
4. Verify: SQLite suite green + Postgres round-trip (`angreal integration up` / `DUALDB_PG_URL`)
   + ≥1 `#[diesel_dualdb::test(pg, sqlite)]`.

Sequencing note: the **pattern is proven** (codegen + dispatched runner compile). The
remainder is mechanical-but-iterative diesel typing; pick up at step 1 above.

### 2026-06-25 — COMPLETE (commit c91f476). All ACs met.
- `Store::migrate` → `weir_schema::migrate` (dispatched per-backend); **0 `sql_query` left in
  weir-engine**; all 13 sites on the typed query builder.
- Upsert: **no `dispatch` needed** — `on_conflict` isn't carried by MultiBackend, so the
  stream_state checkpoint uses a portable **update-then-insert** (safe: a key is written by a
  single claimed worker). The only per-arm code is the DDL pick inside `weir_schema::migrate`.
- **weir-engine SQLite suite: 14 green.** **`weir-schema/tests/dual_backend.rs`
  `#[diesel_dualdb::test(pg, sqlite)]` passes on BOTH** (`_pg ok`, `_sqlite ok`) against the
  `angreal integration` Postgres — migrate idempotency, UUID PKs, `Bytes` opaque, ts-ordered
  recency, the update-then-insert upsert. Workspace builds. Original `type "blob" does not
  exist` crash fixed at the foundation.
- Note vs AC wording: dual-backend proof lives in **weir-schema** (the shared schema all three
  crates use) rather than weir-engine — same portable surface, and it grows as S2/S3 add tables.
