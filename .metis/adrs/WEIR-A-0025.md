---
id: 001-integration-adapter-strategy
level: adr
title: "Integration adapter strategy"
number: 1
short_code: "WEIR-A-0025"
created_at: 2026-06-17T02:12:34.523392+00:00
updated_at: 2026-06-17T02:12:34.523392+00:00
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

# ADR-0025: Integration adapter strategy

**Status:** Proposed. *Raised by: [[WEIR-S-0013]] Integration Adapters.*

## Context **[REQUIRED]**

Adapters (Airflow, DataHub, Superset, Terraform) make weir a first-class citizen of the surrounding stack. They are adoption drivers that must stay API-only, versioned against the API, and independently releasable.

## Decision **[REQUIRED]**

*Proposed:* adapters live as **separate, independently-versioned packages** in their ecosystems (Airflow provider, Terraform provider), coupling only to the Control Plane API ([[WEIR-A-0006]]) and the OpenLineage stream ([[WEIR-A-0022]]). They lean **open** as adoption drivers; polished managed experiences may be vendor offerings (per vision Decision Log).

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Separate, API-only, open (chosen) | Native ecosystem distribution; independent cadence | Several packages to maintain | Low | Medium |
| In-core, monolithic | One release | Couples cadence; bloats core | Medium | Medium |
| Vendor-only | Monetizable | Loses adoption-driver value; against neutrality | Medium | Low |

## Rationale **[REQUIRED]**

Distribution through each tool's own ecosystem (PyPI Airflow provider, Terraform registry) maximizes adoption; API-only coupling keeps them safe and independently releasable. Open baseline drives adoption; managed polish is the vendor's.

## Consequences **[REQUIRED]**

### Positive
- Native distribution; independent release cadence; adoption-driving.

### Negative
- Multiple packages and API-version compatibility to maintain.

### Neutral
- Shares API ([[WEIR-A-0006]]) and lineage standard ([[WEIR-A-0022]]).
