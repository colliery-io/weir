---
id: 001-prior-art-dependency-vs-pattern
level: adr
title: "Prior-art dependency vs. pattern policy"
number: 1
short_code: "WEIR-A-0027"
created_at: 2026-06-17T03:22:40.274353+00:00
updated_at: 2026-06-17T03:23:21.112584+00:00
decision_date: 2026-06-16
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0027: Prior-art dependency vs. pattern policy

**Status:** Decided. *Platform-wide; raised while deciding [[WEIR-A-0002]] and [[WEIR-A-0010]].* *Decision-maker: Dylan Storey, 2026-06-16.*

## Context **[REQUIRED]**

weir leans heavily on Colliery prior art (`fidius`, `cloacina`, `diesel-dual-db`). Two forces make *how* we lean on each a real decision, not a case-by-case reflex:

1. **ASF graduation / IP cleanliness** (vision principles 4–5): the open core is donated to the ASF and must be vendor-neutral with clean IP provenance and permissive dependencies. Pulling a Colliery *application* into the core entangles an ASF-bound codebase with a vendor's product.
2. **The Out-of-Scope boundary** (vision): weir does **not** replace general workflow orchestration (Airflow's job). Depending on a general orchestrator would smuggle that scope back in through the dependency graph.

The decision during [[WEIR-A-0002]]/[[WEIR-A-0010]] — "use cloacina's patterns, do not depend on cloacina" — generalizes into a policy worth stating once.

## Decision **[REQUIRED]**

**Classify each piece of prior art as a *dependency* or a *pattern*, by the following test, and consume it accordingly.**

**Bring in as a dependency** only if all hold:
- It is a **standalone library** (not an application/engine), with a bounded, well-defined surface.
- It is **permissively licensed** with clean, transferable IP provenance (ASF-compatible).
- Its scope sits **inside** weir's scope (it does not import an Out-of-Scope concern).

**Adopt as a pattern only** (reimplement in weir; do not depend) if any hold:
- It is an **application or general engine** (e.g., a general workflow orchestrator).
- Depending on it would **import an Out-of-Scope concern** or couple the ASF-bound core to a vendor application.
- Its IP/licensing provenance can't be made cleanly ASF-transferable.

### Current classification
| Prior art | Kind | Classification | Why |
|-----------|------|----------------|-----|
| `fidius` | Plugin framework (library) | **Dependency** | Bounded library; the connector execution substrate ([[WEIR-A-0002]], [[WEIR-A-0014]]) |
| `diesel-dual-db` | Diesel dual-backend layer (library) | **Dependency** | Bounded library; the store substrate ([[WEIR-A-0009]]) |
| `cloacina` | General orchestrator ("Airflow, but better") | **Pattern only** | Application/general orchestrator; depending would breach Out-of-Scope + entangle the ASF core. Adopt its agent-fleet + transactional-outbox patterns ([[WEIR-A-0002]], [[WEIR-A-0010]]), reimplemented in weir's Sync Engine |

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| Library-vs-pattern test (chosen) | Protects ASF IP cleanliness + Out-of-Scope boundary; still reuses proven designs | Reimplementing patterns is real work | **Chosen** |
| Depend on whatever's proven, incl. cloacina | Less code to write | Breaches Out-of-Scope; ASF IP entanglement; couples core to a vendor app | Rejected |
| Reimplement everything (no prior-art deps) | Maximal independence | Throws away clean, bounded, reusable libraries (`fidius`, `diesel-dual-db`) for no gain | Rejected |

## Rationale **[REQUIRED]**

The test separates "reusable bounded library with clean IP" (safe to depend on) from "application/engine whose scope or provenance is a liability" (adopt the idea, not the artifact). It keeps the open core donatable and inside its declared scope while still standing on proven Colliery designs.

## Consequences **[REQUIRED]**

### Positive
- A single, citable rule for every future prior-art question; protects ASF graduation and the scope boundary.
- Keeps the dependency graph permissive and bounded.

### Negative
- Pattern-only prior art (e.g., cloacina's orchestration) must be **reimplemented** in weir — real engineering cost ([[WEIR-S-0004]]).

### Neutral
- Applies to future prior art too; revisit the classification table as new candidates appear.
- Relates to the open-core boundary ([[WEIR-A-0005]]) and IP/governance workstream ([[WEIR-V-0001]] §Governance).
