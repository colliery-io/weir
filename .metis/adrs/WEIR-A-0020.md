---
id: 001-migration-translation-fidelity
level: adr
title: "Migration translation fidelity"
number: 1
short_code: "WEIR-A-0020"
created_at: 2026-06-17T02:12:24.206847+00:00
updated_at: 2026-06-22T01:10:42.014030+00:00
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

# ADR-0020: Migration translation fidelity

**Status:** Accepted (ratified 2026-06-21). *Raised by: [[WEIR-S-0008]] Migration Importer.*

**Realized by** the parity arc ([[WEIR-I-0008]] declarative coverage + the fidelity harness; the
Python-CDK codemod/adapter follows). The harness (recorded-fixture acceptance tests) is the fidelity
gate this ADR mandates; every translation reports a **tier + confidence** and never silently emits a
broken connector.

## Context **[REQUIRED]**

Airbyte connectors come in four flavors (manifest-only, low-code, Python CDK, Java/Kotlin). "Mechanically translatable" applies to some, not all. We must set the coverage policy and how custom-Python-component connectors are detected and handled.

## Decision **[REQUIRED]**

*Decided:* a **tiered translation policy** — declarative YAML translated automatically (must pass the acceptance-test harness); low-code with embedded Python flagged with adapter scaffolding; Python CDK gets codemod + adapter + porting guide; Java/Kotlin DB/warehouse connectors are out of scope for migration (built first-party). Every output reports a **tier and confidence**; never silently emit a broken connector.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Tiered + confidence reporting (chosen) | Honest; covers the real long tail | Not 100% automated | Medium | Medium |
| Attempt full automation | Maximal coverage claim | False confidence; broken connectors | High | High |

## Rationale **[REQUIRED]**

Migration's real job is the long-tail declarative API sources where translation works. Transparency about tiers preserves trust; the harness ([[WEIR-S-0006]]) is the fidelity gate.

## Consequences **[REQUIRED]**

### Positive
- Credible migration story without overpromising.

### Negative
- Manual porting remains for custom-Python and CDK connectors.

### Neutral
- Targets the contract ([[WEIR-A-0014]]); strategy per [[WEIR-A-0003]].
