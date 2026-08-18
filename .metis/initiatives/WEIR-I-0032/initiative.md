---
id: tech-debt-connector-conformance
level: initiative
title: "Tech-debt: connector conformance test kit (host-side read/write seam)"
short_code: "WEIR-I-0032"
created_at: 2026-07-08T14:58:58.128712+00:00
updated_at: 2026-07-08T14:58:58.128712+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: tech-debt-connector-conformance
---

# Tech-debt: connector conformance test kit (host-side read/write seam) Initiative

> **Tech-debt ticket** (2026-07-08 architecture review, "Worth exploring"). This is parked in discovery on
> purpose — its exit criterion is a **decision to promote for fix**, not the fix itself.

## Context

Connectors are `cdylib`, so they can't `cargo test`. The pure logic — the `test_decoding` CDC parser, the `pg_cdc`
SQL builders, `render_path`, `StreamSchema::infer` — is therefore *exiled* into `weir-connector-types` where it has
unit tests, each called back from exactly one guest. But a connector's *actual* `read()`/`write()` behaviour — the
socket/HTTP loop, pagination, checkpoint emission — is only exercised end-to-end by loading the compiled `.wasm` in
`weir-engine/tests/wasm_*_engine.rs`. There is **no in-process seam** to drive a connector's read/write with a fake
egress; the trait lives behind the plugin macro.

So "is this connector correct?" is answered in a *different* crate (`connector-types`, for parsing) and a *third*
crate (`weir-engine` integration, requiring a wasm build, for behaviour) — you bounce between three places to reason
about one connector, and "the interface is the test surface" is violated. The explore pass judged this a genuine
**missing seam** (the deletion test mostly *moves* — you can't drop the hoisted logic without losing the only fast
tests). It maps directly onto the drafted **[[WEIR-A-0031]]** ("Connector conformance test kit").

## Goals & Non-Goals

**Goals:**
- Reach a **decision**: is a host-side connector-under-test seam worth building now?

**Non-Goals:**
- Building the kit. That work belongs to the promoted initiative *if* the decision is yes.

## Detailed Design

*Decision input, not a build.* If promoted, the likely shape is a conformance harness (per [[WEIR-A-0031]]) that
stages a connector and drives its `read()`/`write()` through the fidius boundary with fixture input and a captured/
faked egress, asserting behaviour **through the interface** — plus, optionally, splitting each connector into a
testable inner `lib` behind the thin `cdylib` shell so the exiled logic can come home. The cost is a new testkit
surface + per-connector fixtures; the payoff is locality and one harness that checks every connector's contract.

## Alternatives Considered

- **Do nothing.** Keep the hoist + whole-engine integration tests. Cheapest; leaves the middle untested. A valid
  outcome of the decision.
- **Only split connectors into inner libs (no harness).** Restores unit-testability of logic without a fidius-driven
  behavioural seam. A middle option to weigh in the decision.

## Implementation Plan

Single step — **decide**:
- [x] **Make a decision to promote for fix**: either (a) promote to a fix initiative (build the [[WEIR-A-0031]]
  conformance kit), or (b) close, recording the reason in an ADR so future architecture reviews don't re-suggest it.

## Decision (2026-08-17)

**Promote — post-alpha.** Build a **thin v1** of the [[WEIR-A-0031]] conformance kit (spec/check/discover/read against author fixtures + golden outputs, on the existing `weir-wasm-testkit` build/stage seam) as the trust gate for community connectors when the catalog opens. It is explicitly **not alpha-gating**: the alpha catalog is first-party, and the alpha's misconfiguration pain is addressed by creation-time validation ([[WEIR-T-0166]]) and the live suite ([[WEIR-I-0014]]). Sequencing: after the alpha-cut initiatives ([[WEIR-I-0042]]–[[WEIR-I-0047]]); the fix initiative should be created from [[WEIR-A-0031]]'s open design questions (fixture/golden format, record/replay, publish-gate policy). Additional consumer discovered since filing: [[WEIR-A-0041]] names the conformance kit as the compensating control for guest-side TLS verification. This ticket's exit criterion is met.
