---
id: live-schedule-registration-sub
level: task
title: "Live schedule registration + sub-second (fractional) intervals"
short_code: "WEIR-T-0051"
created_at: 2026-06-24T02:07:44.523332+00:00
updated_at: 2026-06-24T03:31:01.804017+00:00
parent: WEIR-I-0010
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0010
---

# Live schedule registration + sub-second (fractional) intervals

*This template includes sections for various types of tasks. Delete sections that don't apply to your specific use case.*

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0010]]

## Objective **[REQUIRED]**

Surfaced while demoing [[WEIR-I-0010]]: a connection added live in the UI never schedules, and intervals are
whole-seconds only. Two real gaps (the scheduler engine + `every_secs`/`cron` already exist — WEIR-T-0009/
T-0020):
1. **Live schedule registration** — `register_schedules` runs once at `serve` startup with a non-idempotent
   `add`, so UI-added/edited connections aren't picked up until restart. Add a reconcile that re-syncs the
   `schedules` table against the connections each `serve` loop.
2. **Sub-second intervals** — `every_secs` is `u64` + `Duration::from_secs`; the scheduler already stores
   `every_ms`, so widen `every_secs` to **f64** (down to **0.1s**) end-to-end + a sub-second `serve` poll.
3. **Demo shows it** — enable SQLite WAL (concurrent access) + run `weir serve` (scheduler) alongside the
   `weir api` UI; seed connections with intervals; drop the fake `/run` driver.

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

### 2026-06-24 — DONE (commits 2942e4f, 296b20c + fixes)
**1. Fractional intervals:** `every_secs` → **f64** (≥0.1s) end-to-end (Connection DOUBLE col, DTO, CLI
`--every`, UI parse); `register_schedules`/`sync_schedules` use `Duration::from_secs_f64`; CLI `serve --poll`
fractional. (Scheduler already stored `every_ms`.)

**2. Live registration:** `Scheduler::schedules()`/`remove()` + `App::sync_schedules` reconcile the schedule
set vs the live connections each `serve` loop (add new / re-register changed cadence / drop removed) — UI
add/edit/delete picked up **without a restart**.

**3. In-app scheduler:** `weir api` spawns `App::serve` (100ms poll) in-process — scheduler+worker live in
the app, not an external driver. Demo is fire-and-forget (launch detached, seed, exit; leave weir running);
the fake `/run` loop is gone. `App::serve` loop is **resilient** (logs+continues; a transient error no
longer strands runs in `pending`).

**4. Concurrency knob:** `WorkerConfig.concurrency` (**default 16**) caps simultaneous executions —
orthogonal to worker count; atomic claim keeps concurrent claimants safe (verified; cf. cloacina
outbox+claim). `run_until_idle` drains up to N concurrently (FuturesUnordered, refill-on-processed);
`drain()` stays serial (sync helper). Plumbed via `App::serve`/`weir_api::serve`/CLI `--concurrency` (api+serve).

**5. SQLite contention (cloacina CLOACI-T-0622):** diesel-dualdb can't set per-connection `busy_timeout`,
so **retry-on-'database is locked'** (30s budget, exp backoff) wraps the worker claim/apply + the engine
checkpoint transaction.

**Tests:** `sync_picks_up_live_connection_at_subsecond_interval` (live pickup + 0.1s granularity +
idempotent reconcile); existing partitioned/retry tests now exercise the concurrent drain. 44 groups / 52
tests green, clippy clean. Verified live: scheduler drives the demo on its own; cold-start drains
continuously.

### 2026-06-24 — throughput fix (commit c7794ee)
Concurrency exposed that throughput was **~0.2 runs/s (4030 ms/run)** — `from_wasm_package` JIT-compiled the
wasm component (`Component::new`) on **every run**.
- **Benchmark** `tests/throughput.rs` (echo→arrow-sink, K=1/4/16) measured it first.
- **Fix — connector-handle cache:** `ConnectorRef::resolve` caches `Arc<ConnectorHandle>` per
  `(search_path, package, config)`; compile once, reuse across runs (each run still instantiates a fresh
  sandbox Store — `ConnectorHandle` is `Send+Sync`, verified). **0.2 → 22.5 runs/s @K=1, 295 @K=4** (~110×).
- **Cache invalidation (user ask):** `ConnectorRef::invalidate_cache(path, package)` evicts all configs for
  a package; called on `App::import` (re-upload/sync) + `App::unregister_connector`.
- **Substrate confirmed:** tokio multi-thread runtime + blocking pool (512) >> concurrency 16 — threads
  aren't the limit. Remaining ceilings (follow-ups, not blocking): unbounded handle cache (LRU later);
  SQLite single-writer (Postgres for write scale); precompiled `.cwasm` (~83µs load) to cut first-compile.
45 groups green, clippy clean. **Task complete.**
