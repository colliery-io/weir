---
id: f1-long-lived-source-runtime
level: initiative
title: "F1 — Long-lived source runtime (continuous polling / event-reader sources)"
short_code: "WEIR-I-0035"
created_at: 2026-07-09T03:19:40.423717+00:00
updated_at: 2026-07-14T09:58:14.093747+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: f1-long-lived-source-runtime
---

# F1 — Long-lived source runtime (continuous polling / event-reader sources) Initiative

> **Feature request — Signal Fabric enablement (F1 of 6).** Source: "weir — Feature Requests to Support the
> Signal Fabric" (Swish Exchange ADR-007 realizes the Signal Fabric *as* weir). **Home: open-core.** This is the
> foundational feature — [[WEIR-I-0036]] (F2), [[WEIR-I-0038]] (F4), [[WEIR-I-0039]] (F5), [[WEIR-I-0040]] (F6)
> all assume it. **Phase: design** — design proposed below, pending ratification, then decompose.

## Context

A source execution mode where a source process **stays resident and emits continuously**, rather than running to
completion and exiting. Two sub-shapes under one runtime:

- **Continuous-polling source** — holds its configuration and connection, polls its upstream on a declared cadence,
  and emits a record each cycle (or on change, if the source supports change detection).
- **Event-reader source** — subscribes to or tails a live upstream (socket, stream, changefeed) and emits records
  as they arrive.

Both are declared the way existing weir sources are (connector config), with an added lifecycle:
`start → run (indefinitely) → stop`, supervised, with restart-on-failure and backoff.

**Grounding in what weir already has.** [[WEIR-A-0029]] (decided) already evolved the connector contract to
**streaming reads + configured instances**: `read(ReadContext) -> Stream<ReadMessage>`, config bound once at
construction, and *"CDC becomes a long-lived change stream."* So the "process stays resident and emits
continuously" data-path substrate largely exists. **F1 is predominantly the supervised lifecycle and resource
governance around that contract** — not a second connector system and not a from-scratch runtime.
See [[WEIR-S-0005]] (Connector Runtime / Worker) and [[WEIR-S-0004]] (Sync Engine).

**Charter note.** The weir vision ([[WEIR-V-0001]]) scoped *sub-second true streaming* out. F1 stays on the right
side of that line: **"stay up and emit," not "compute over a stream"** — no windowing, joins, or aggregation in the
runtime. The vision's Constraints section is amended in this program to reflect the narrowed boundary.

## Goals & Non-Goals

**Goals:**
- A connector can be declared a **resident source** and run indefinitely under supervision, reusing the existing
  connector contract and SDK — resident-vs-run-to-completion is a **runtime mode**, decided by the runtime.
- Supervised lifecycle: crash → **restart with backoff**; a source must never silently die.
- **Resource-bounded** residency: declared memory/connection ceilings so 100s co-scheduled don't exhaust a host.
- A polling source emits on its declared cadence; an event-reader emits on upstream arrival.

**Non-Goals:**
- High-throughput stream *processing* — windowing, joins, aggregation in the runtime (stays out, per [[WEIR-V-0001]]).
- The fan-out destination ([[WEIR-I-0036]]) and its scale properties ([[WEIR-I-0038]]) — separate initiatives.
- The freshness triple ([[WEIR-I-0037]]) — separate contract work (next in sequence).

## Open-Core Boundary

**Wholly open-core.** A long-lived source runtime is a general capability that strengthens weir for anyone wanting
live sources, independent of Swish. Nothing fabric-specific lives here.

## Detailed Design

> **Grounded in a code pass over `weir-engine`, `weir-runtime`, `weir-orchestrator`, `weir-app` (2026-07-09).**
> Central finding: **residency reuses the existing orchestrator machinery almost wholesale.** The orchestrator is a
> transactional-outbox + lease-based work queue (cloacina pattern): `Relay` state machine
> (`pending → leased → done/failed/requeued`), `Relay::heartbeat` extends a lease, `Worker::tick` requeues
> `Transient` errors with exponential backoff, `Fleet` runs one worker per tenant, and leader-election already
> exists (`Relay::try_acquire_lease`). Because of [[WEIR-A-0029]], a source's streaming `read` for a resident
> source simply *never returns end-of-stream*. So the new engineering is small and **additive** — the
> run-to-completion path is untouched.

### The model: a resident source is a never-completing work unit

A resident source is enqueued **once** as a work unit that stays `leased` with a **perpetual heartbeat** and runs a
streaming read loop that never ends. This lets us reuse, unchanged:
- the per-tenant claim/lease/heartbeat machinery,
- `Transient`-error **requeue with exponential backoff** as the restart mechanism,
- the per-tenant runner `Fleet` and Kubernetes actuator.

Genuinely new (all additive branches): the `ExecutionMode` declaration, "skip the scheduler + enqueue-once" for
resident units, perpetual-lease semantics + a `resident` marker (observability), a resource-budget admission check,
and a cooperative stop/cancel.

### Declaration surface (resolves OQ2)

- **Capability on the connector `spec()`** — a connector advertises whether it *can* run resident (and whether it's
  an event-reader). Advertised, not authored twice.
- **Intent + tuning on the connection config** (`weir-app` `Connection`) — a new `execution_mode` field, parsed in
  `work_spec()` into a `WorkSpec` enum:
  ```
  ExecutionMode::RunOnce
  ExecutionMode::Resident { cadence_ms: Option<u64>,   // polling; None for event-reader
                            memory_ceiling_mb: u32,
                            connection_ceiling: u32,
                            restart_backoff_ms: u64 }
  ```
  Mode is a per-connection operational property (like `sync_mode`), so the same connector can run batch or resident
  by how it's wired. `validate_connection_modes` is extended to reject incoherent combinations.

### Lifecycle, supervision & failure (resolves OQ1 + OQ3)

- **Reschedulable, NOT pinned (OQ1).** No sticky-runner/affinity. A resident source is a work unit kept alive by
  heartbeat; if its runner (Pod) dies, the lease expires and **any runner in the same tenant fleet re-claims it**,
  resuming from the last committed checkpoint. This reuses all existing machinery and avoids building
  leader-per-source affinity. Cost: a small replay window on runner death — acceptable under at-least-once
  ([[WEIR-A-0011]]).
- **Scheduler skipped for resident mode.** Not interval/cron-fired; enqueued once as a standing registration until
  explicitly stopped. `Relay::has_active` already prevents double-start.
- **Restart = the existing backoff path (OQ3).** On crash / stream error the stream ends; `Worker::tick` requeues
  with `base_delay * (1 << attempt)` and re-claims; execution resumes from the last checkpoint. Same at-least-once
  guarantee, existing mechanism. "Must never silently die" is met by requeue + provable liveness (heartbeat now;
  F3's liveness heartbeat later).
- **Checkpoints are source-driven.** The connector emits `ReadMessage::Checkpoint` at its own granularity (contract
  already supports it, [[WEIR-A-0029]]); the resident loop commits transactionally on each — no new checkpoint
  mechanism, just committing mid-stream instead of at end-of-stream.

### The drive loop

`Engine::sync_with` already loops over the `read` stream and commits transactionally on `Checkpoint`. The resident
variant is mostly that loop **without teardown on stream-end** plus a **cooperative stop signal**. Refactor to share
the loop between run-once and resident; add an `Engine` resident entry that holds the connector handle open and
drains indefinitely. Connector handles are already cached (`HANDLE_CACHE`), so keeping one warm is natural.

### Resource governance — measured & reactive (revised 2026-07-09 per review)

*Not* pre-emptive per-source declared budgets — those are guesses. Instead, instrument the running sources and scale
reactively:
- **Instrument actual usage.** The runner reports real mem/cpu (per-runner; best-effort per resident source) via the
  observability path ([[WEIR-A-0022]]) into health ([[WEIR-S-0011]], `weir-app/health.rs`).
- **Reactive claim headroom.** A worker stops claiming *new* resident sources once its **measured** usage crosses a
  high-water mark — load spreads to runners with headroom instead of piling on a hot one. (Admission keyed on
  measured current usage, not declared budgets.)
- **Reactive scale-out** ("hit X mem/cpu → launch another instance"). When the tenant fleet is saturated, the
  **existing autoscaler** (`weir-orchestrator/src/autoscaler.rs` → `actuator.rs`, the `weir-runner-{tenant}`
  Deployment) adds an instance; new residents land there. This is weir's "K8s autoscaling in the open core"
  differentiator ([[WEIR-V-0001]]) applied to residency.
- **Rebalancing a hot runner (follow-on).** Because residency is **unpinned** (lease-based), a hot runner can shed
  load by releasing some resident leases; they expire and re-claim onto a less-loaded instance. MVP relies on
  new-load-spreading + autoscale; active shedding is a follow-on if relieving an already-hot runner proves needed.
- **Ceilings are runner-level high-water thresholds** (config), not required per-source fields; a per-source hint
  stays optional. No wasm-instance/connection **pooling** yet (a later scale optimization).

### Design decisions (forks resolved)

| Fork | Decision | Why |
|---|---|---|
| Persistent instance | Refactor engine to share the loop; resident holds the handle open, no teardown | Loop already works; change is lifecycle, not a new engine |
| Checkpoint granularity | Source-driven via `ReadMessage::Checkpoint` | Contract already supports; connector owns cadence |
| Supervision/restart | Reuse `Worker::tick` `Transient` requeue + backoff | Don't rebuild a supervisor; the work queue *is* one |
| Lease/heartbeat | Stay `leased`, perpetual heartbeat + a `resident` marker | Reuse machinery; marker keeps health/UI honest |
| Runner affinity | **No pinning** — reschedulable, resume from checkpoint | Reuses everything; small replay window is acceptable |
| Resource model | **Measured + reactive**: instrument real mem/cpu; stop claiming at high-water; autoscale the fleet (shed via lease-expiry as follow-on) | Declared per-source budgets are guesses; reuse the existing autoscaler + the unpinned lease model |
| Scheduler | Skip for resident; enqueue-once registration | "Active" is the steady state for a resident source |

### Key seams (`file:symbol`)

| Component | Seam | F1 change |
|---|---|---|
| Mode declaration | `weir-app/src/lib.rs:Connection` (~:70); `validate_connection_modes` | add `execution_mode` + validation |
| WorkSpec routing | `weir-app/src/lib.rs:work_spec` (~:1746) | parse mode → `ExecutionMode` enum |
| Worker dispatch | `weir-orchestrator/src/lib.rs:Worker::tick` (~:940) | branch resident vs run-once |
| Engine loop | `weir-engine/src/lib.rs:Engine::sync_with` (~:694, checkpoint :725) | share loop; resident no-teardown + stop |
| Supervision | `weir-orchestrator/src/lib.rs:Worker::tick` requeue (~:1047); `Relay::heartbeat` (~:661) | perpetual lease; requeue-on-exit |
| Scheduler | `weir-orchestrator/src/lib.rs:Scheduler::tick` (~:1266) | skip resident mode |
| Resource instrumentation | `weir-orchestrator/src/autoscaler.rs`; `weir-app/src/health.rs`; claim loop | measured mem/cpu; high-water claim gate → autoscaler |

## Alternatives Considered

- **A second, stream-only connector system.** Rejected by the memo's constraint: authors write **one** kind of
  connector; residency is a runtime decision.
- **Unsupervised long-running processes.** Rejected: "must not silently die" is an explicit acceptance bar.
- **Sticky runner affinity / leader-per-source (pinning).** Rejected for MVP — far more to build (affinity owner,
  failover) than reschedule-from-checkpoint, which the existing lease machinery gives us for free.
- **A bespoke resident supervisor process.** Rejected — the lease-based work queue already *is* a supervisor; a
  parallel one would duplicate claim/heartbeat/backoff.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A connector can be declared a resident source and run indefinitely under supervision.
- [ ] A polling source emits on its declared cadence; an event-reader emits on upstream arrival.
- [ ] Killing the upstream or the source process results in **supervised restart** (requeue + resume from
  checkpoint), not a dead source.
- [ ] As resident sources accumulate, a runner stays within its resource envelope — it stops taking new residents at
  its high-water mark and the **fleet scales out** rather than OOMing.

## Open Questions (resolved in design)

- **Residency vs scheduler/leader-election + runner fleet — pinned or reschedulable?** → **Reschedulable**, no
  pinning; re-claim + resume from checkpoint; scheduler skipped for resident mode.
- **Declaration surface for cadence + ceilings?** → **Capability on `spec()`; intent + cadence + ceilings on the
  connection config**, threaded via `WorkSpec` `ExecutionMode`.
- **Backoff/restart vs at-least-once checkpoints?** → **Reuse the existing `Transient` requeue + exponential
  backoff**; resume from last source-driven checkpoint (semantics unchanged, [[WEIR-A-0011]]).

*Still genuinely open (defer to implementation / follow-on):* active **lease-shedding rebalance** of an
already-hot runner; wasm-instance & connection **pooling** (scale optimization); exact `resident`
state-vs-marker representation.

## Implementation Plan

**Exit criteria (design → ready → decompose):** design ratified with the human (this section); charter boundary
confirmed against amended [[WEIR-V-0001]]. Candidate task decomposition:

1. **ExecutionMode model + threading** — `Connection.execution_mode` + `spec()` capability + `WorkSpec`
   `ExecutionMode` enum + validation. (`weir-app`, `weir-connector-types`)
2. **Resident engine loop** — refactor `Engine::sync_with` to share the drive loop; resident entry (no teardown,
   commit on source `Checkpoint`, cooperative stop). (`weir-engine`)
3. **Resident supervision in the worker** — `Worker::tick` resident branch; perpetual heartbeat; requeue-on-exit
   with backoff; skip scheduler; enqueue-once + `resident` marker. (`weir-orchestrator`)
4. **Resource instrumentation + reactive scaling** — per-runner (best-effort per-source) mem/cpu into health; a
   measured **claim-headroom gate**; wire the high-water trigger to the existing autoscaler; surface resident
   liveness to health ([[WEIR-I-0024]]). (`weir-orchestrator` autoscaler/actuator, `weir-app/health.rs`)
5. **Resident integration + soak coverage** — polling + event-reader resident fixtures; kill-upstream → restart
   test; N-resident **scale-out** test (crosses high-water → autoscales, not OOM). (build on [[WEIR-I-0023]] soak +
   `weir-engine` tests)

## Status Updates

**2026-07-09 — F1 execution (Ralph, branch `weir-f1-resident-source-runtime`).**
- ✅ **F1.1 [[WEIR-T-0137]] COMPLETE** — `ExecutionMode` + `Connection.execution_mode` + `work_spec` threading +
  `0003_resident_runtime` schema. Build/clippy/fmt green; changed-crate lib tests green. Connector `spec()`
  capability **re-homed to F3 ([[WEIR-I-0037]])** (breaking `WitType` wire change); schema `generated/` hand-written
  (reviewer to re-run `angreal schema gen`).
- ✅ **F1.2 [[WEIR-T-0138]] COMPLETE** — `weir-engine` shared `drive()` + `run_resident()` + `stop_channel()`
  (drop-to-cancel) + `EngineError::ResidentStreamEnded`. Build green; engine lib 14 + integration tests pass; no
  run-once regressions; dependency direction preserved.
- ✅ **F1.3 [[WEIR-T-0139]] COMPLETE** — worker resident branch → `run_resident` + `StopHandle` registry; perpetual
  lease (free from existing heartbeat); unbounded requeue-with-backoff on exit; scheduler skips resident. Build green;
  orch lib 5 + scheduler 6 pass. **Deferred → F1.5:** enqueue-once START path + manual-STOP API wiring (`weir-app`).
- ✅ **F1.4 [[WEIR-T-0140]] COMPLETE** — `UsageProbe` (injectable) + claim-headroom gate (`Relay::release`) +
  autoscaler saturation nudge; metrics `weir_runner_usage_fraction`/`weir_resident_gated`. Build/clippy(-D) green;
  orch lib 7 + scheduler 7 pass. **Deferred → F1.5:** real probe, production wiring (`with_headroom`/`with_saturation`
  + runner→control-plane usage report), health.rs fields.
- ✅ **F1.5 [[WEIR-T-0141]] COMPLETE** — START/STOP control plane (`App::start`/`stop`, `weir start`/`stop`,
  `--execution-mode`) **plus** the re-homed tail: real `SysUsageProbe` (`/proc`, dependency-free) wired via
  `Fleet::with_headroom` + autoscaler `SaturationFn` (env `WEIR_RESIDENT_HIGH_WATER`); `health.rs` `RunnerUsage`
  fields; behavioral tests (resident stream-end→requeue w/ checkpoint intact; worker high-water gate→release;
  health status). Build green (default + `--features kubernetes`); engine 5 / scheduler 8 / weir-app lib 24 pass.
  **Residual follow-ons (tracked in T-0141, not delivered):** dedicated wasm cadence/event fixtures; docker soak;
  cross-process mid-stream stop; per-connection cadence/backoff; per-tenant usage.

## F1 end-state (2026-07-09)

**Landed green on branch `weir-f1-resident-source-runtime` (uncommitted):** the full resident *path* — declare
(`execution_mode`) → engine `run_resident` loop (stop/checkpoint) → worker supervision (perpetual lease, requeue,
scheduler-skip) → reactive-scaling *logic* → **launch/stop** control plane. Every step builds green with unit/
integration tests; run-once untouched.

Real resource-probe wiring + health surface and the core behavioral coverage (restart-on-stream-end with checkpoint
intact; high-water gate→release through the worker; health status) **now landed and green** (F1.5 pass 2).

**Docker resident soak DELIVERED + PASSING** (pass 3): `/connections/{name}/start`+`/stop` API routes +
`weir-soak --resident N` (resident liveness + enqueue-once invariants, segregated from the scheduled gates).
`angreal soak --mode full --resident 4` → `resident=4/4 (overrun=0)` every window, all invariants PASS
(verified by my own 30s run + the fork's 90s/628-run run). Resident sources ran supervised under the provisioned
Postgres stack: stream-end → requeue → re-run, never silently dying.

**Residual follow-ons (tracked in [[WEIR-T-0141]]):** dedicated wasm cadence/event-arrival fixtures (behavioral
coverage now substantial via the soak, so this is a nicety); cross-process mid-stream stop; per-connection
cadence/backoff; per-tenant usage. Carried to **F3**: connector `spec()` resident-capability + the `validate` gate.

**Verdict:** all five F1 tasks complete; the resident *path* is wired end-to-end, green (unit + integration), and
**soak-verified under docker**. **Schema regenerated + verified** — `angreal schema gen` (crates.io
`diesel-dualdb-cli`) reproduced the previously hand-written `weir-schema/generated/` exactly (schema.rs = the two
`execution_mode -> Text` columns; per-backend `0003` migrations regenerated); build/tests green after regen. Remaining
reviewer to-do before merge: review the diff (~730 lines core + API/soak). Nothing committed.

## Goal scorecard (2026-07-10, F1.6 [[WEIR-T-0142]] — all verified by own runs)

| Goal | Status | Evidence |
|---|---|---|
| **1 — declare + run resident under supervision** | ✅ MET | soak: residents run supervised; reuses existing contract |
| **2 — supervised, never silently die** | ✅ MET | `resident_reclaims_on_runner_death_by_a_different_runner` (lease-expiry → re-claim, resume from checkpoint) + `run_resident_errs_on_stream_end...`; **cooperative-stop bug fixed** (was starved by a hot stream) |
| **3 — resource-bounded / scale-out** | ✅ MET | `--resident 10 --fleet 8` co-exist healthy (10/10 alive, batch flowing) + `--resident 50` alive/bounded, **no pool exhaustion / OOM** (after the pool-hold fix); `saturated_fleet_scales_out_through_the_actuator` proves scale-out |
| **4 — cadence polling / event-reader emission** | ✅ MET | `resident_polling_emits_on_declared_cadence` + `resident_event_reader_emits_per_arrival` (real wasm fixtures) |

**Two defects found + fixed while closing the goals:** (1) cooperative-stop starvation in `Engine::drive` (hot
resident stream never observed drop-to-cancel — an F1 acceptance was silently broken); (2) resident pool-connection
hold (perpetual runs exhausted the DB pool under co-scheduled load). Both fixed + regression-green.

**Remaining follow-ons (out of F1's goal scope, tracked):** mixed batch+resident *throughput* at high N on a single
node needs the proven autoscaler (more runners) or a Postgres control plane — a multi-runner soak is the follow-on;
cross-process mid-stream stop; per-tenant usage; and the connector `spec()` capability re-homed to **F3**.

**F1 GOALS: all four met, evidence in hand. Still uncommitted — human diff review + commit/PR to close the
initiative.**

## F1.7 [[WEIR-T-0143]] — ws→ws resident consumer (interface validation + extension) ✅

Brought in to stress the resident interfaces with a real **non-poll** consumer. Result: the interfaces carry it.
- Decision [[WEIR-A-0039]] (brokered ws egress) — corrected during the work to be *cheaper*: the guest already has
  host-brokered raw TCP (`fidius_guest::sockets::tcp::connect` + `EgressPolicy::authorize_tcp`), so a ws is RFC6455
  over that — **no new host capability for `ws://`**; connector stays wasm.
- Built: `wasm-fixtures/resident-ws/` (hand-rolled RFC6455, dependency-free, wasm-builds) + e2e vs a local echo ws
  server — **3/3 pass** (arrival-emission, close→transient→reconnect, zero-arrivals-zero-records). Regression green.
- Confirms the resident/event-reader/streaming/supervision interfaces hold for a live socket consumer.
- **Follow-ons (tracked in T-0143):** guest can't interrupt a *blocking* socket read mid-frame (stop honored only
  between frames); `wss`/auth (host-brokered handshake); ws sink e2e; egress scheme tag.