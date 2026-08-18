---
id: f4-fan-out-at-scale-from-a-single
level: initiative
title: "F4 — Fan-out at scale from a single source read"
short_code: "WEIR-I-0038"
created_at: 2026-07-09T03:19:40.569295+00:00
updated_at: 2026-07-09T03:19:40.569295+00:00
parent: WEIR-V-0001
blocked_by: [WEIR-I-0035, WEIR-I-0036]
archived: true

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: f4-fan-out-at-scale-from-a-single
---

# F4 — Fan-out at scale from a single source read Initiative

> **Feature request — Signal Fabric enablement (F4 of 6).** **Home: open-core** (a performance/scale property of the
> F2 destination + F1 runtime). Depends on [[WEIR-I-0035]] (F1) and [[WEIR-I-0036]] (F2). **Not a new surface** — a
> stated **scale requirement** on the F1 runtime + F2 destination together. Filed in **discovery**.
>
> Fan-out at scale is a measurable property of the **Signal Broker** ([[WEIR-S-0017]]); its execution model is
> [[WEIR-A-0038]]. The NFRs here map to REQ/NFR-S17.* in that spec.

## Context

One resident source runs the expensive upstream read (SQL query, API call) **once per cycle**, and the result
reaches **many** bound subscribers cheaply. This is the capability that makes the **read-amplification collapse**
real: replacing *"1000 consumers each run the query"* with *"one source runs it, 1000 subscribers read the
result."*

It is kept as its own initiative (not folded into F2) because its acceptance is a distinct, **measurable** bar —
sustained fan-out ratio and per-subscriber overhead — that gates whether F1+F2 are "done" for the fabric, and
because it is the initiative that carries the **TBD scale target** to be set with Swish E&I.

## Goals & Non-Goals

**Goals:**
- The expensive work (the upstream read) is paid **once per source cycle**, independent of subscriber count.
- **Per-subscriber delivery cost is low and roughly linear** — adding subscribers does not re-trigger the source
  read or degrade source cadence.
- Sustain a **target fan-out ratio** — **[TARGET TBD: consumers per signal — set with Swish E&I; order of magnitude
  is 100s–1000s of subscribers per source].**
- Slow-subscriber backpressure (from F2) stays **isolated** — it must not couple back into source read cadence.

**Non-Goals:**
- A new API/destination surface — this hardens and measures F1+F2; it introduces no new connector-facing interface.
- Setting the numeric target unilaterally — the ratio and the per-subscriber overhead ceiling are set **with Swish
  E&I** and recorded here.

## Open-Core Boundary

**Wholly open-core.** "One read, many cheap deliveries" is a general scale property valuable to anyone running live
sources with many consumers. No fabric-specific logic lives here.

## Detailed Design

*(Discovery-phase sketch.)*

- **Decoupling.** The source read cadence (F1) and the per-subscriber delivery path (F2) must be provably
  decoupled: subscriber count is invisible to the read loop; deliveries fan out from a single produced batch.
- **Measurement harness.** This initiative is largely **validation + hardening**: a soak/benchmark harness
  (building on [[WEIR-I-0023]] soak) that scales subscribers against a fixed source and asserts (a) read frequency
  is flat vs. subscriber count, (b) source cadence holds, (c) per-subscriber overhead stays within the target.

## Alternatives Considered

- **Fold into F2 as acceptance criteria.** Considered and set aside (per structuring decision): the measurable
  scale bar and the TBD target with Swish E&I are substantial enough to track distinctly, and F4 gates F2's fabric
  readiness rather than being a sub-check of it.
- **Per-consumer source reads (status quo of naive designs).** Rejected — this is exactly the read amplification the
  feature exists to collapse.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] Adding subscribers to a running source does **not** increase upstream read frequency.
- [ ] Source read cadence holds steady as subscriber count scales to the target ratio.
- [ ] Measured per-subscriber overhead stays within **[TARGET TBD]** at the target fan-out.

## Open Questions

- **[TARGET TBD]** — the concrete fan-out ratio and per-subscriber overhead ceiling (set with Swish E&I). Blocking
  for the third acceptance criterion; not blocking for standing up the harness.
- What hardware/topology profile the target is measured against (single worker vs. runner fleet, [[WEIR-A-0036]]).

## Implementation Plan

Discovery deliverable — confirm F1/F2 shapes support the decoupling, agree the TBD target with Swish E&I, then
decompose. Candidate seams: (1) benchmark/soak harness for fan-out scaling; (2) instrumentation proving read
frequency is flat vs. subscriber count; (3) cadence-hold + per-subscriber-overhead assertions against the target.

**Exit criteria:** F1+F2 designs confirmed to support the guarantee; scale target agreed and recorded; harness
approach ratified with the human; decomposable into tasks.
