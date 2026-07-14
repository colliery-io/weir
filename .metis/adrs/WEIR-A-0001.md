---
id: 001-core-language-split
level: adr
title: "Core language split"
number: 1
short_code: "WEIR-A-0001"
created_at: 2026-06-17T02:11:17.483261+00:00
updated_at: 2026-06-17T18:06:12.830341+00:00
decision_date: 2026-06-17
decision_maker: Dylan Storey
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0001: Core language split

**Status:** Decided. *Raised by: all core components.* *Decision-maker: Dylan Storey, 2026-06-17.*

## Context **[REQUIRED]**

weir needs a language for the control plane (scheduler, sync engine, worker orchestration, autoscaler) and a language for connector authoring. A lean per-worker runtime — low memory footprint and fast cold-start — is a control-plane goal, which motivates Rust over a JVM-based worker model. Separately, the connector ecosystem (Airbyte CDK, Singer, dlt, Meltano) and AI-assisted authoring are overwhelmingly Python.

## Decision **[REQUIRED]**

**Rust** for the control plane and runtime host; **Python** for the connector SDK and builder. Connectors are artifacts behind a stable contract ([[WEIR-A-0014]]), so the host language and the authoring language are decoupled.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| Rust core + Python SDK (chosen) | Lean binary, K8s-operator fit; meets connector authors in Python | Team Rust depth; two-language build | Medium | Medium |
| All-Python | One language; fastest authoring | Resource weight, weak concurrency story for the core | Medium | Low |
| All-Rust | Maximal leanness | Forfeits the Python connector ecosystem — the largest asset | High | High |

## Rationale **[REQUIRED]**

The connector ecosystem is the single largest asset in this space; forcing another language on authors would forfeit it. The core's resource profile is a real differentiator, where Rust wins. The contract seam makes the split clean.

## Consequences **[REQUIRED]**

### Positive
- Lean, predictable core; natural Kubernetes operator fit.
- Connector authors stay in Python; AI-assisted authoring is well-supported.

### Negative
- Two-language toolchain and build complexity.
- Requires sufficient Rust depth on the core team (pressure-test per vision Decision Log).

### Neutral
- Native (Rust) connectors remain a separate question ([[WEIR-A-0016]]).
- **Reinforced by downstream decisions:** the contract seam is concrete — `fidius` provides the dylib + WASM execution primitives and the PyO3 boundary for first-party Python ([[WEIR-A-0002]]); the connector contract is a `fidius` interface with manifest→codegen authoring ([[WEIR-A-0014]]). The split is no longer just a recommendation — its mechanism is decided.
- Standing watch item (from [[WEIR-V-0001]] Decision Log): core-team Rust depth; ratification does not retire this — it is a staffing/risk concern, not a reason to reopen the decision.
