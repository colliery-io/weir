---
id: 001-secrets-backend-abstraction
level: adr
title: "Secrets backend abstraction"
number: 1
short_code: "WEIR-A-0021"
created_at: 2026-06-17T02:12:25.845948+00:00
updated_at: 2026-08-18T01:59:25.782515+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/superseded"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0021: Secrets backend abstraction

**Status:** Superseded by [[WEIR-A-0037]] (closed 2026-08-17, Dylan Storey). *Raised by: [[WEIR-S-0010]] Secrets Manager.*

> **Closure note (2026-08-17):** [[WEIR-A-0037]] decided that secrets are **off-platform — consumed, not managed**: weir reads secrets from where the operator's secret system already puts them (env / mounted file / injected connection config) and re-reads per run. Under that decision there is no secrets *backend* to abstract — env/file consumption **is** the interface, and Vault/KMS integration belongs to the operator's tooling (external-secrets, mounted files), not to weir. The pluggable-backend trait proposed below is therefore not built. The user-facing follow-through (redact-on-read, write-only secret fields, `env:`/`file:` references) is scoped in [[WEIR-I-0047]].

## Context **[REQUIRED]**

Dev wants zero-setup secrets; enterprises want their own Vault/KMS. The Secrets Manager must support pluggable backends behind one interface so callers (Engine/Runtime) never change.

## Decision **[REQUIRED]**

*Proposed:* a **backend trait/interface** with implementations for env/file (dev), and Vault / cloud KMS / cloud secret managers (prod). Callers use handles ([[WEIR-A-0013]]); the backend is a deployment-time choice.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Pluggable backend interface (chosen) | Dev-trivial + enterprise-grade; no caller change | Interface must cover rotate/revoke/audit | Medium | Medium |
| Single built-in store | Simple | No enterprise vault story | Medium | Low |

## Rationale **[REQUIRED]**

A narrow pluggable interface contains the most sensitive data and lets enterprises bring their own KMS without weakening the open core.

## Consequences **[REQUIRED]**

### Positive
- Open-core dev simplicity; enterprise backends without forking.

### Negative
- Interface must abstract rotation/revocation/audit uniformly.

### Neutral
- Resolution path per [[WEIR-A-0013]].
