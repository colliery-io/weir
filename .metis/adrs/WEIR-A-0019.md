---
id: 001-connector-versioning-compatibility
level: adr
title: "Connector versioning & compatibility policy"
number: 1
short_code: "WEIR-A-0019"
created_at: 2026-06-17T02:12:21.765173+00:00
updated_at: 2026-06-22T18:50:19.071234+00:00
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

# ADR-0019: Connector versioning & compatibility policy

**Status:** Accepted (2026-06-22). *Raised by: [[WEIR-S-0007]] Catalog, [[WEIR-S-0006]] Connector Contract & SDK.* Decided: **per-connection version pinning** (no deployment-wide active version), pinned-by-default with first-class operational hold-back, contract-range compatibility gating.

## Context **[REQUIRED]**

Connectors evolve and the contract ([[WEIR-A-0014]]) is versioned. Users and the Runtime need a predictable policy for which connector version is compatible with which contract version, plus deprecation rules.

## Decision **[REQUIRED]**

1. **Connectors are semver-versioned.** Each connector *version* is an immutable artifact; the catalog
   stores **all registered `(name, version)` pairs** — multiple versions coexist (wasm packages keyed by
   name+version; cheap to hold side-by-side).
2. **Pinning is per-connection and explicit.** A connection **freezes the connector version it was created
   with** — `ConnectorRef` carries a `version`. There is **no deployment-wide "active"/latest** that
   connections float on. Reproducibility is by construction: a connection runs the same version until a
   human changes it.
3. **No auto-upgrade; hold-back is first-class.** A newer version appearing in the catalog **never** alters
   a running connection. Upgrading is a deliberate per-connection edit (re-pin to a new version). A
   connection may stay on an older version **indefinitely** for operational reasons — the Airflow
   core/providers model: you pin, and you choose when to move.
4. **Contract-compatibility gate.** Each connector version declares the **contract-version range**
   ([[WEIR-A-0014]] / [[WEIR-A-0029]]) it supports. The Catalog enforces it at **registration** and the
   Runtime at **dispatch** — a pinned version incompatible with the running engine is refused/flagged, not
   silently run. A stated **deprecation window** applies to contract major bumps.
5. **Origin/trust recorded per version** (first-party vs community), feeding the isolation tier
   ([[WEIR-A-0016]]). The UI may *surface* "newer version available" but never applies it.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **Per-connection pinning (chosen)** | Max reproducibility; per-pipeline hold-back + gradual upgrade; parity can register new versions without disturbing anyone | No single "what's everyone on" view; version travels in `ConnectorRef` everywhere | Low | Medium |
| Per-deployment single active (Airflow-strict) | One installed version; simplest schema | All-or-nothing upgrades; no per-pipeline hold-back | Med | Low |
| Hybrid (deployment default + per-conn override) | Convenient default | Dual-mode complexity; "active" drift | Med | Med |
| Latest-only | Simple | Breaks reproducibility/pinning | High | Low |

## Rationale **[REQUIRED]**

Per-connection pinning makes every run reproducible by default and lets operators upgrade (or hold back) one
pipeline at a time — essential when the parity arc ([[WEIR-I-0008]]) is registering connector versions on
an external (Airbyte) cadence the operator doesn't control. wasm coexistence makes "many versions installed"
cheap, so the single-active-version convenience isn't worth its drift + all-or-nothing upgrades. Contract-range
gating lets the contract evolve without silently breaking pinned connectors.

## Consequences **[REQUIRED]**

### Positive
- Reproducible runs; safe, gradual, per-pipeline upgrades; first-class operational hold-back.
- Parity registers new versions without touching existing connections.

### Negative
- No single "what version is everything on" — must query per connection.
- `version` must travel in `ConnectorRef` + the catalog everywhere; existing connection rows need a
  version backfill on migration.
- Authors carry a contract-range declaration burden.

### Neutral
- Enforced by the Catalog ([[WEIR-S-0007]] / [[WEIR-I-0010]]); the artifact channel that feeds versions is
  [[WEIR-A-0018]] (local dir now).
