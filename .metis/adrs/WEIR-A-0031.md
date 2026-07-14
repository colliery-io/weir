---
id: 001-connector-conformance-test-kit
level: adr
title: "Connector conformance test kit"
number: 1
short_code: "WEIR-A-0031"
created_at: 2026-06-22T19:18:03.822470+00:00
updated_at: 2026-06-22T19:18:03.822470+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/draft"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0031: Connector conformance test kit

**Status:** Proposed (draft — needs design). *Raised by: [[WEIR-S-0014]] Connector Developer Experience
(REQ-1.3).*

## Context **[REQUIRED]**

[[WEIR-S-0014]] requires that an author gain **confidence a connector is correct before distributing it**.
Today that's ad-hoc (per-connector engine tests + the in-repo test connectors); there is no uniform bar a
community-authored connector can be held to. With **WASM-always** ([[WEIR-A-0030]]) every connector is a
wasm package, and with **per-connection pinning** ([[WEIR-A-0019]]) operators rely on "this `(name,
version)` behaves" — both want a **conformance gate**. Airbyte's Connector Acceptance Tests (CAT) are the
reference model.

## Decision **[REQUIRED]**

*Proposed:* a **`weir connector test`** conformance kit that runs a connector (wasm package) through the
contract surface against author-provided fixtures + **golden outputs**, runnable locally and in CI:

- **spec/check** — `spec()` yields a valid `config_schema`; `check(config)` passes/fails as declared.
- **discover** — `discover(config)` returns the expected stream catalog.
- **read** — `read(ctx)` over a recorded fixture yields golden records + checkpoints; cursor advances;
  dead-letters/logs match expectations.
- **write** — `write(ctx, batches)` returns the expected receipt + dead-letters.
- **contract conformance** — declared `contract_range` ([[WEIR-A-0019]]) is satisfied; streaming surface
  honors backpressure/cancel ([[WEIR-A-0029]]).
- **Determinism** — network sources use recorded fixtures (VCR-style record/replay), so the kit is
  hermetic + reproducible.

Open for design: the golden-file/fixture format, the record/replay mechanism, and whether passing is
**required** to publish / register a community connector (trust gate, ties to [[WEIR-A-0030]] origin).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk Level | Implementation Cost |
|--------|------|------|------------|-------------------|
| **weir-native conformance kit (chosen direction)** | Uniform bar; one command; CI-able; trust gate for community connectors | Authors write fixtures/goldens; build a record/replay harness | Medium | Medium |
| Ad-hoc per-connector tests (status quo) | No new tooling | No uniform bar; can't gate a community catalog | High (trust) | Low |
| Port Airbyte CAT wholesale | Proven coverage | Heavy; coupled to Airbyte's protocol shape | Medium | High |

## Rationale **[REQUIRED]**

A uniform, runnable conformance bar is the **trust mechanism** for an open catalog: an operator installing
a community connector can rely on "passed the kit," and an author gets fast local feedback. It is the
confidence gate the rest of the lifecycle ([[WEIR-S-0014]]) assumes.

## Consequences **[REQUIRED]**

### Positive
- Confidence-before-ship for authors; a trust signal for operators; CI-enforceable quality bar.

### Negative
- Authors must supply fixtures + golden outputs; weir must build + maintain a record/replay harness.

### Neutral
- May become a **required gate** for publish/registration (decide with [[WEIR-A-0018]] / [[WEIR-A-0030]]);
  realized as part of the [[WEIR-S-0014]] tooling, alongside `weir connector new|build|publish`.
