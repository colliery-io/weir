---
id: 001-open-core-periphery-boundary
level: adr
title: "Open-core / periphery boundary"
number: 1
short_code: "WEIR-A-0005"
created_at: 2026-06-17T02:11:53.128898+00:00
updated_at: 2026-08-18T01:59:21.662050+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0005: Open-core / periphery boundary

**Status:** Decided (2026-08-17, Dylan Storey) — ratified as proposed. *Raised by: all. Ratification recorded during the alpha decision-closure pass; the drafted decision below stands unchanged.*

## Context **[REQUIRED]**

The ASF makes the boundary explicit and unforgiving: an Apache project ships everything under Apache 2.0 and is vendor-neutral. We must decide how extension points are drawn and packaged so the open core stands alone and the proprietary periphery attaches without crippling the center.

## Decision **[REQUIRED]**

*Proposed:* the moat is **architectural and operational**, not a feature held hostage. Every extension point (connectors, plugins, integrations, deployment) is a designed, versioned interface; the periphery attaches through those interfaces in separate repos. The open core must be a product someone would happily run in production for free.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Operational/architectural moat (chosen) | ASF-compatible; ethical; community-friendly | Requires disciplined interface design | Medium | Medium |
| Feature-gated open-core | Easy monetization | Incompatible with ASF governance | High | Low |

## Rationale **[REQUIRED]**

The same interface that lets the community contribute is the one that lets a vendor sell; ASF discipline forces it to be clean. The periphery is operating-it-at-scale (operator, autoscaler, governance, managed control plane), which the ASF has no objection to a vendor selling.

## Consequences **[REQUIRED]**

### Positive
- "Cannot be enclosed" becomes a structural, leadable differentiator.

### Negative
- Demands rigorous interface design across every component.

### Neutral
- Packaging coupling with deployment ([[WEIR-A-0023]]) and the RBAC seam ([[WEIR-A-0008]]).
