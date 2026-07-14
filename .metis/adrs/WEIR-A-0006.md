---
id: 001-api-protocol-versioning
level: adr
title: "API protocol & versioning"
number: 1
short_code: "WEIR-A-0006"
created_at: 2026-06-17T02:11:54.870508+00:00
updated_at: 2026-06-17T02:11:54.870508+00:00
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

# ADR-0006: API protocol & versioning

**Status:** Proposed. *Raised by: [[WEIR-S-0002]] Control Plane, [[WEIR-S-0013]] Integration Adapters.*

## Context **[REQUIRED]**

The Control Plane API is the single surface for the UI and all integration adapters, which are pure clients. We must choose the protocol(s) and a backward-compatibility/versioning policy that adapters can depend on.

## Decision **[REQUIRED]**

*Proposed:* REST/JSON as the primary, broadly-consumable surface; evaluate gRPC for internal/high-throughput paths. Backward-compatible within a major version; explicit deprecation policy.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| REST/JSON | Ubiquitous; trivial for adapters/Terraform | Verbose; weaker streaming | Low | Low |
| gRPC | Efficient, typed, streaming | Heavier client story for some integrations | Medium | Medium |
| Both | Best fit per consumer | Two surfaces to maintain | Medium | Medium |

## Rationale **[REQUIRED]**

Adapters (Airflow, Terraform, DataHub) and the UI are the primary clients; REST maximizes reach. Versioning discipline protects the independently-released adapters.

## Consequences **[REQUIRED]**

### Positive
- Low-friction integration surface; stable contract.

### Negative
- Possible dual-protocol maintenance if gRPC is added.

### Neutral
- Shared by Integration Adapters ([[WEIR-S-0013]]).
