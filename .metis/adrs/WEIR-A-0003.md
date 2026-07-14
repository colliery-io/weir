---
id: 001-airbyte-compatibility-strategy
level: adr
title: "Airbyte compatibility strategy"
number: 1
short_code: "WEIR-A-0003"
created_at: 2026-06-17T02:11:48.794068+00:00
updated_at: 2026-06-22T01:10:40.532468+00:00
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

# ADR-0003: Airbyte compatibility strategy

**Status:** Accepted (ratified 2026-06-21). *Raised by: [[WEIR-S-0008]] Migration Importer, [[WEIR-S-0006]] Connector Contract & SDK.*

**Partially realized + sequenced:** [[WEIR-T-0012]]/[[WEIR-T-0013]] established the declarative-YAML → weir-manifest importer (a thin first slice: bearer/api-key auth, basic pagination, datetime cursor). The parity arc deepens it — declarative coverage first ([[WEIR-I-0008]]), then the Python-CDK codemod/adapter (a later initiative) — under the tiered fidelity policy of [[WEIR-A-0020]].

## Context **[REQUIRED]**

We want Airbyte's connector ecosystem without inheriting its protocol constraints. Two strategies: run Airbyte connectors unmodified (runtime wire-compatibility) or own a clean format and provide migration tooling.

## Decision **[REQUIRED]**

**Migration, not runtime compatibility.** Own a clean connector contract ([[WEIR-A-0014]]) designed from scratch, and ship a migration importer ([[WEIR-S-0008]]): mechanical translation of declarative YAML connectors, codemod + adapter for Python CDK.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Migration importer (chosen) | Clean owned protocol; pave over friction; inherit long tail by translation | Migration tooling effort; not 100% coverage | Medium | Medium |
| Runtime wire-compatibility | Run connectors unmodified | Inherit Airbyte's protocol constraints permanently | High | High |

## Rationale **[REQUIRED]**

This lets us be a credible replacement AND redesign the protocol (typed schemas, robust checkpointing, reverse-ETL semantics) without being bound to Airbyte's wire format. Migration's real job is the long-tail API sources where translation works; databases/warehouses are built first-party regardless.

## Consequences **[REQUIRED]**

### Positive
- Freedom to design the contract; no legacy protocol debt.

### Negative
- Migration is tiered; not all connectors translate mechanically (see [[WEIR-A-0020]]).

### Neutral
- Sets the contract ([[WEIR-A-0014]]) as the translation target.
