---
id: 001-api-protocol-versioning
level: adr
title: "API protocol & versioning"
number: 1
short_code: "WEIR-A-0006"
created_at: 2026-06-17T02:11:54.870508+00:00
updated_at: 2026-08-18T01:59:23.056168+00:00
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

# ADR-0006: API protocol & versioning

**Status:** Decided (2026-08-17, Dylan Storey). *Raised by: [[WEIR-S-0002]] Control Plane, [[WEIR-S-0013]] Integration Adapters.*

## Context **[REQUIRED]**

The Control Plane API is the single surface for the UI and all integration adapters, which are pure clients. We must choose the protocol(s) and a backward-compatibility/versioning policy that adapters can depend on.

## Decision **[REQUIRED]**

**Decided:** REST/JSON is the API surface. **Alpha stability statement (2026-08-17):** the API is **v0/unstable** — breaking changes are allowed between releases and must be called out in the CHANGELOG; no compatibility promise is made to integrators during alpha. Freezing a backward-compatible `/api/v1` is a **beta gate**, not an alpha requirement. The gRPC question (internal/high-throughput paths) is explicitly deferred until after the v1 freeze. Once `/api/v1` exists: backward-compatible within a major version, explicit deprecation policy, adapters version against it.

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
