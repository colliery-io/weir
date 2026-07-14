---
id: foundations-design
level: initiative
title: "Foundations & Design"
short_code: "WEIR-I-0001"
created_at: 2026-06-17T02:00:00.250806+00:00
updated_at: 2026-06-17T02:00:00.250806+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: XL
initiative_id: foundations-design
---

# Foundations & Design Initiative

*Phase 0 of the weir vision ([[WEIR-V-0001]]). This is a heavy, deliberately front-loaded planning initiative. While the overall delivery is "agile," we plan a significant amount of work before we start building — because the connector contract and execution model are simultaneously the moat boundary, the community contribution surface, and the integration surface, and getting them wrong is expensive.*

## Context **[REQUIRED]**

The weir vision ratifies *what* we are building and *why*, and the strategic bets behind it. It does not resolve the design. This initiative produces the next two rungs of the top-down decomposition:

> vision (why, where we play) → **capabilities (what the platform can do)** → **components (the building blocks)** → **interfaces / decisions (the contracts and ADRs between them)**

The output documents are the durable inputs to every downstream implementation initiative. We are explicitly *design-first*: we nail the connector contract (ADR-0014) and the execution/isolation model (ADR-0002) before committing to implementation, because they gate the Runtime, SDK, Catalog, and Migration Importer together.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- Ratify the vision ([[WEIR-V-0001]]) with founding stakeholders.
- Produce the **Capabilities Catalog** ([[WEIR-S-0001]]) — the implementation-neutral "what it can do" rung.
- Produce **12 component specifications** (WEIR-S-0002 … WEIR-S-0013) — one per building block, each with functional/non-functional requirements and the ADRs it forces.
- Produce the **25 ADRs** (WEIR-A-0001 … WEIR-A-0025) surfaced by the component specs, written up from context → options → decision → consequences, starting with the first wave.
- Establish IP hygiene and contributor-agreement structure from commit one (ASF-readiness).
- Run the formal name/trademark clearance for **weir**.

**Non-Goals:**
- No implementation of data movement in this initiative — design only.
- No resolution of the open-core *vendor* periphery internals (separate vendor repos).
- No roadmap dates — sequencing only; dates come with the downstream implementation initiatives.

## Detailed Design **[REQUIRED]**

This initiative is a documentation/design effort. Its "design" is the structure of the artifact set it produces and how they feed forward.

**Artifact set (this initiative's deliverables):**

1. **Capabilities Catalog** — [[WEIR-S-0001]]. Capability areas A–M, tagged Core / Periphery / Core→Periphery and P0/P1/P2, plus explicit out-of-scope.
2. **Component specs** — twelve specifications, each from the component PRDs:

   | Spec | Component |
   |------|-----------|
   | WEIR-S-0002 | Control Plane / API |
   | WEIR-S-0003 | Web UI |
   | WEIR-S-0004 | Sync Engine (Orchestrator) |
   | WEIR-S-0005 | Connector Runtime (Worker) |
   | WEIR-S-0006 | Connector Contract & SDK |
   | WEIR-S-0007 | Connector Catalog / Registry |
   | WEIR-S-0008 | Migration Importer |
   | WEIR-S-0009 | Metadata & State Store |
   | WEIR-S-0010 | Secrets Manager |
   | WEIR-S-0011 | Observability & Lineage |
   | WEIR-S-0012 | Deployment & Operator |
   | WEIR-S-0013 | Integration Adapters |

3. **ADRs** — WEIR-A-0001 … WEIR-A-0025, the decisions the component specs force. One decision per ADR; referenced by id everywhere; immutable once accepted (a changed decision gets a new ADR that supersedes it).

**Feed-forward model.** Once these specs reach a ratified state, each component spec (and its resolved ADRs) becomes the feeder for a downstream *implementation* initiative on the roadmap. The capabilities catalog's P0/P1/P2 tags and the ADR first-wave ordering set the implementation sequence.

**First-wave decisions** (unblock the most downstream work, in rough order): ADR-0014 (connector contract) and ADR-0002 (execution model) → then ADR-0009 / ADR-0010 (state store, work distribution) → then ADR-0007 (data model).

## Alternatives Considered **[REQUIRED]**

- **Big PRD up front, then design.** Rejected: the contract is the load-bearing artifact; a monolithic PRD would freeze surface decisions before the contract that constrains them exists. We do design-first (vision Decision Log).
- **Skip the planning rung, start coding the Rust core.** Rejected: the execution/isolation model is unproven at our scale and the connector contract is the moat boundary — committing in code before the ADRs is the expensive mistake this initiative exists to avoid.
- **One combined design document.** Rejected: per-component specs give traceability and let each feed an independent downstream initiative; ADRs as separate immutable records survive supersession cleanly.

## Implementation Plan **[REQUIRED]**

1. **Vision ratified** — [[WEIR-V-0001]] reviewed with stakeholders. *(in progress)*
2. **Capabilities catalog** — WEIR-S-0001 authored and reviewed.
3. **Component specs** — WEIR-S-0002 … WEIR-S-0013 authored; each lists the ADRs it raises.
4. **ADRs** — WEIR-A-0001 … WEIR-A-0025 created; first wave (0014, 0002, 0009, 0010, 0007) written up and moved toward Decided.
5. **IP / governance scaffolding** — CLA / Software Grant structure, LICENSE/NOTICE hygiene, name clearance.
6. **Hand-off** — ratified specs + decided first-wave ADRs feed the Phase-1 implementation initiatives.

## Exit Criteria

- [ ] Vision ratified (WEIR-V-0001 published).
- [ ] Capabilities catalog complete (WEIR-S-0001).
- [ ] All 12 component specs authored (WEIR-S-0002…0013).
- [ ] All 25 ADRs created; first-wave ADRs Decided.
- [ ] IP/contributor-agreement structure in place; weir name clearance run.
