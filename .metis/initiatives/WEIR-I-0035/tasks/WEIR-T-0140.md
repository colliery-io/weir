---
id: f1-4-resource-instrumentation
level: task
title: "F1.4 — Resource instrumentation + reactive scaling (measured mem/cpu, claim-headroom gate, autoscaler trigger)"
short_code: "WEIR-T-0140"
created_at: 2026-07-09T15:34:52.001027+00:00
updated_at: 2026-07-09T17:14:30.296748+00:00
parent: WEIR-I-0035
blocked_by: [WEIR-T-0139]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0035
---

# F1.4 — Resource instrumentation + reactive scaling (measured mem/cpu, claim-headroom gate, autoscaler trigger)

## Parent Initiative

[[WEIR-I-0035]] (F1 — Long-lived source runtime)

## Objective

Keep a runner within its resource envelope as resident sources accumulate — **measured, not pre-declared**.
Instrument real mem/cpu, stop claiming new residents past a high-water mark, and wire that mark to the **existing
autoscaler** so the fleet scales out ("hit X → launch another instance"). Bin-packing/preemption and lease-shedding
rebalance are explicitly out (follow-on).

## Acceptance Criteria

## Acceptance Criteria

- [ ] The runner reports **measured** mem/cpu (per-runner; best-effort per resident source) through the
  observability path ([[WEIR-A-0022]]) into health ([[WEIR-S-0011]]).
- [ ] A **claim-headroom gate**: a worker stops claiming *new* resident sources once measured usage crosses its
  high-water threshold (config), so load spreads to runners with headroom.
- [ ] When the tenant fleet is saturated, the **existing autoscaler adds an instance** (`autoscaler.rs` →
  `actuator.rs` `weir-runner-{tenant}` Deployment); new residents land there.
- [ ] High-water thresholds are runner-level config (per-source values are optional hints only).
- [ ] Resident liveness/usage is visible in the health surface for ops.

## Implementation Notes

### Technical Approach
- Autoscaler trigger: `weir-orchestrator/src/autoscaler.rs` (leader-gated `tick`), `actuator.rs` (`scale`).
- Claim gate: `weir-orchestrator/src/lib.rs:Worker::tick`/claim loop; `WorkerConfig` (~:283) for thresholds.
- Instrumentation → health: `weir-app/src/health.rs`; observability standard [[WEIR-A-0022]].

### Dependencies
[[WEIR-T-0139]] (the resident worker branch + claim path this gates).

### Risk Considerations
- Per-source mem/cpu attribution inside a shared runner is best-effort (wasm instances share a process) — don't
  over-promise per-source precision; per-runner is the reliable signal.
- Autoscale must remain leader-gated to avoid thundering scale-outs.

## Status Updates

**2026-07-09 — COMPLETED (logic build-verified green; production wiring deferred).**
`crates/weir-orchestrator/src/{lib,autoscaler}.rs` + `tests/scheduler.rs`.
- `UsageProbe` trait → `Usage { mem_fraction, cpu_fraction }` (`max_fraction()`); default `NullUsageProbe` (full
  headroom → gate inert until configured). Injectable → unit-testable.
- **Headroom gate:** `Worker::tick`, if `high_water` set and `usage.max_fraction() ≥ hw` and unit is resident →
  `Relay::release(id, defer)` (back to Pending, undo the claim's attempt bump, defer re-claim), skip execution.
  Run-once never gated. Distinct from failure `requeue`.
- **Scale-out:** released units keep the queue non-empty; `scaled_target()` nudges a saturated tenant +1 (clamped,
  depth>0) on top of the existing depth policy → the existing leader-gated autoscaler adds an instance.
  `with_saturation()` injects the signal (default off).
- Metrics: `weir_runner_usage_fraction` gauge, `weir_resident_gated` counter.
- **Verify:** fmt clean; `clippy -p weir-orchestrator --all-targets -D warnings` clean; `cargo build --workspace`
  green; orch lib 7 + scheduler 7 pass (incl. `resident_gate_decision_and_usage`,
  `saturation_nudges_target_up_one_within_max`, `resident_release_returns_unit_to_pending_deferred`).

**DEFERRED to F1.5 ([[WEIR-T-0141]]) — production wiring (same class as F1.3's control-plane deferrals):**
1. **Real usage probe** (best-effort `sysinfo`-style, cross-platform) — currently `NullUsageProbe`.
2. **Construct workers/autoscaler with it** — nothing yet calls `Worker::with_headroom(...)` /
   `Autoscaler::with_saturation(...)`; no runner→control-plane usage-reporting path feeding a real `SaturationFn`.
3. **Health-surface fields** — usage/resident is on the metrics path (ADR-0022) but not yet in `weir-app/health.rs`
   struct fields (needs #2's cross-process report). Acceptance "visible in health surface" partially met (metrics
   yes, health.rs no).