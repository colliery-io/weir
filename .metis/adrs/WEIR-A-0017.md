---
id: 001-ai-assisted-authoring-approach
level: adr
title: "AI-assisted authoring approach"
number: 1
short_code: "WEIR-A-0017"
created_at: 2026-06-17T02:12:17.643204+00:00
updated_at: 2026-06-17T02:12:17.643204+00:00
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

# ADR-0017: AI-assisted authoring approach

**Status:** Proposed. *Raised by: [[WEIR-S-0006]] Connector Contract & SDK.*

## Context **[REQUIRED]**

The connector ecosystem has shifted toward agent-generated pipelines (capabilities §E). We want AI-assisted authoring as a differentiator, with guardrails so generated connectors are correct and safe.

## Decision **[REQUIRED]**

*Proposed:* design the contract and SDK to be **AI-legible** (typed, declarative-first, self-describing) and provide an authoring loop where an agent scaffolds a connector from an API spec, then the **acceptance-test harness** ([[WEIR-S-0006]]) gates correctness before catalog publication.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| AI-legible contract + test-gated authoring (chosen) | Meets authors where they are; safety via conformance | Requires strong harness + provenance | Medium | Medium |
| Manual authoring only | Predictable | Misses the ecosystem shift | Medium | Low |

## Rationale **[REQUIRED]**

The harness makes AI authoring safe: generation is cheap, conformance is the gate. A declarative-first contract is the most agent-friendly target.

## Consequences **[REQUIRED]**

### Positive
- Lowers the contribution barrier; rides the agent-authoring trend.

### Negative
- Conformance harness and provenance/IP hygiene become load-bearing.

### Neutral
- Depends on contract design ([[WEIR-A-0014]]).
