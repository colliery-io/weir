---
id: s4-postgres-on-wasm-wasi-sockets
level: task
title: "S4: Postgres on wasm (WASI sockets)"
short_code: "WEIR-T-0044"
created_at: 2026-06-22T20:03:37.436324+00:00
updated_at: 2026-06-23T20:03:16.347351+00:00
parent: WEIR-I-0011
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0011
---

# S4: Postgres on wasm (WASI sockets)

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0011]]

## Objective **[REQUIRED]**

S4 of [[WEIR-I-0011]] — **the hard spot, sequenced separately** (runs in parallel / may trail the S3 flip;
must not gate the rest). Bring the `postgres` connector up on wasm: a wasm-compatible pg driver over **WASI
sockets**, with an integration test against a real Postgres. This is the residual native-dependency risk
[[WEIR-A-0002]] flagged. **If blocked** by a WASI/fidius capability gap, document the gap precisely and
raise it as a (user-owned) fidius FR; postgres may lag behind the flip until resolved. AC: `postgres` runs
as a wasm package against a live DB **or** a documented blocker + FR exists.

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

### 2026-06-23 — UNBLOCKED upstream; waiting on fidius 0.5.4 (watcher set)
The blocker (a wasm guest can't open raw TCP — fidius brokered HTTP only) is **solved upstream** by
**fidius PR #4 / FIDIUS-I-0033** — *"capability-gated TCP egress for sandboxed guests."* It adds
`fidius_guest::sockets::tcp` (blocking `TcpStream` over `std::net` = `wasi:sockets` on wasm32-wasip2;
`Read+Write` so rustls composes), a new `tcp` capability, and `EgressPolicy::authorize_tcp(&SocketAddr)`
(default-deny, connect-only, resolve-and-pin). Exactly what S4 needs.

**Waiting on:** the **fidius 0.5.4** crates.io release carrying that PR. A background watcher polls
`crates.io/api/v1/crates/fidius/0.5.4` (currently 404; latest 0.5.3) and re-invokes on publish.

**Resume plan when 0.5.4 lands:**
1. Bump weir's `fidius` dep 0.5.3 → 0.5.4 (and any pinned 0.5.3 in the wasm fixtures); rebuild green.
2. **Postgres wasm guest** — a `wasm-fixtures/postgres` guest (rest/slow guest shape) using a
   **pure-Rust sync** pg driver (rust-postgres `postgres` crate) over `fidius_guest::sockets::tcp`, with
   `rustls` for TLS. (NOT libpq/tokio-postgres — C/async won't cross-compile to wasm.)
3. weir host: an `EgressPolicy` impl overriding `authorize_tcp` to allow-list the connection's DB
   `host:port` (mirrors the rest http egress); grant the `tcp` capability on the postgres package.
4. Integration test against a real Postgres (the `angreal integration` harness brings one up); re-add
   postgres to the catalog as a wasm connector. Closes the last native-crate carve-out.

### 2026-06-23 (later) — fidius 0.5.4 shipped + integrated; S4 feasibility PROVEN
- **fidius 0.5.4 published** (watcher fired) → bumped all pins; full workspace suite **green** on 0.5.4;
  guests rebuilt + round-trip through the 0.5.4 host (commit 2327c0a).
- **API confirmed:** `fidius_guest::sockets::tcp::connect(host, port) -> io::Result<TcpStream>` (blocking;
  `Read+Write` so rustls + byte drivers compose); host `EgressPolicy::authorize_tcp(&SocketAddr)`
  (default-deny, connect-only, resolve-and-pin); package capability `tcp`.
- **Feasibility EMPIRICALLY PROVEN:** a probe crate using `postgres-protocol` (`startup_message` codec) +
  `fidius_guest::sockets::tcp::connect` **compiled clean for wasm32-wasip2**. postgres-protocol is pure-Rust
  (hmac/sha2/md-5/rand — no ring/openssl/C), so md5/SCRAM auth works in the sandbox. **Correction to the
  plan above:** the `postgres` 0.19 crate is NOT usable (pulls tokio+mio) — the guest hand-rolls a sync
  client over `postgres-protocol` + `sockets::tcp` (no tokio). Probe removed after proving the build.

**Remaining = the pg wire client (no blockers, just implementation):** a `wasm-fixtures/postgres` guest
(contract scaffolding like the others) whose `read` = connect via `sockets::tcp` → `postgres-protocol`
startup + auth → simple Query → parse `RowDescription`/`DataRow` → emit `RecordBatch::Rows(JSON)`; weir host
`EgressPolicy` allow-listing the DB `host:port`; package `capabilities=["tcp"]`; integration test against the
`angreal integration` Postgres; re-add postgres to the catalog. Substantial but fully de-risked.

### 2026-06-23 — DONE: postgres runs as a wasm guest against a live DB (AC met; commit 50c77b1)
- **`wasm-fixtures/postgres`** — a source connector: `read` connects via `fidius_guest::sockets::tcp`, does
  the Postgres startup + auth (cleartext / md5 / **SCRAM-SHA-256**, all via `postgres-protocol`), runs the
  configured simple `Query`, parses `RowDescription`/`DataRow` (text), emits `RecordBatch::Rows(JSON)` + a
  checkpoint. Pure-Rust, no tokio/libpq → compiles clean to wasm32-wasip2.
- **`wasm_postgres_engine`** integration test (`#[ignore]`, needs `docker compose up`): stages the guest
  with `capabilities=["tcp"]` + a `PgEgress` policy (`authorize_tcp` allows `:5432`, denies http), loads via
  `from_wasm_package_with_egress`, runs `SELECT … generate_series(1,2)` through the engine → **2 rows**.
  **Verified green** against `postgres:16` (SCRAM auth over wasm `sockets::tcp`).
- **AC met:** postgres runs as a wasm package against a live DB. The DB connector class (A-0002's original
  native-path justification) now runs fully sandboxed in wasm.

**Follow-ups (new work, not S4):** the wasm guest is a **v1 simple-query source**. The native
`weir-connector-postgres` crate (kept) still has **incremental cursor / CDC / partitions / write-upsert** the
wasm guest lacks — porting those to the wasm guest (→ feature parity, then delete the native crate) is the
remaining postgres work, tracked separately from this S4 (which proved the wasm-DB path).
