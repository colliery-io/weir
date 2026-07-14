---
id: explanation-architecture-models
level: task
title: "Explanation (architecture + models)"
short_code: "WEIR-T-0128"
created_at: 2026-07-08T02:10:53.882073+00:00
updated_at: 2026-07-08T03:00:08.130560+00:00
parent: WEIR-I-0027
blocked_by: [WEIR-T-0125]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0027
---

# Explanation (architecture + models)

## Parent Initiative

[[WEIR-I-0027]] — the "why", understanding-oriented. Closes the initiative.

## Objective

The discursive pages that build a mental model: how weir is put together and why the key design choices are what
they are. Draw on the ADRs, but write for a reader, not as an ADR.

## Reference

- ADRs: control-plane/store ([[WEIR-A-0009]]), work distribution ([[WEIR-A-0010]]), delivery + checkpoints
  ([[WEIR-A-0011]]), WASM-always ([[WEIR-A-0030]]), secrets never in the guest ([[WEIR-A-0033]]), secrets
  off-platform ([[WEIR-A-0037]]), tenancy ([[WEIR-A-0036]]), deployment ([[WEIR-A-0023]]), UI ([[WEIR-A-0035]]).
- Typed schemas + evolution ([[WEIR-I-0025]]); the SQLite-WAL finding ([[WEIR-I-0023]]) for the deployment page.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Architecture** (`explanation/architecture.md`) — the pieces + how a sync flows through them.
- [x] **The connector model** (`connector-model.md`) — WASM sandbox, capabilities/egress, secrets-off-platform,
  manifests vs crates, and why WASM over native/subprocess.
- [x] **Multi-tenancy** (`multi-tenancy.md`) — isolation at execution (per-tenant runner), not just queries.
- [x] **Delivery guarantees** (`delivery-guarantees.md`) — transactional checkpoints, at-least-once + idempotent
  = effectively-once, leases, dead-letters.
- [x] **Typed schemas** (`typed-schemas.md`) + **Deployment topology** (`deployment.md`, incl. the SQLite WAL
  finding) — discursive, not steps.
- [x] `mkdocs build --strict` clean.

## Status Updates

### 2026-07-08 — done (`cc511d1`), closes I-0027

Six explanation pages, drawn from the ADRs but written for a reader (the *why*). Full four-mode docs
(Tutorials / How-to / Reference / Explanation) build `--strict` clean. **Complete — closes [[WEIR-I-0027]].**
