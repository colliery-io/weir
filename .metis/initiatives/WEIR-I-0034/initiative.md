---
id: tech-debt-egress-policy-per
level: initiative
title: "Tech-debt: egress policy — per-connection allow-list or collapse the seam"
short_code: "WEIR-I-0034"
created_at: 2026-07-08T14:59:00.497631+00:00
updated_at: 2026-07-08T14:59:00.497631+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
initiative_id: tech-debt-egress-policy-per
---

# Tech-debt: egress policy — per-connection allow-list or collapse the seam Initiative

> **Tech-debt ticket** (2026-07-08 architecture review, "Speculative"). Parked in discovery on purpose — its exit
> criterion is a **decision to promote for fix**, not the fix itself. Flagged *handle with care*: acting here can be
> premature.

## Context

`EgressPolicy` is a fidius trait with exactly **one** production impl — `HostAllowList` — plus one test-only
(`PgEgress`). And `HostAllowList::allow_all()` is the value at essentially every call site (~25 uses); the only real
policy today is "allow everything, optionally inject a credential." The credential is built at exactly one production
site (`orchestrator`: `Credential::from_auth_config(&config.json)` → `allow_all().with_credential(c)`); the
per-connection allow-list is an explicit TODO. By the rule *"one adapter = a hypothetical seam, two = a real one,"*
this seam hasn't earned its generality yet.

The **sharper** friction the explore pass identified isn't the seam depth — it's the **string-key coupling**:
`from_auth_config` is the *reader* of the same `auth_scheme` vocabulary that `manifest_stream_to_config` *writes*
(see [[WEIR-I-0033]]), and that vocabulary is duplicated in three places (`weir-app` ×2, `weir-runtime`). That's the
cross-cutting theme — *stringly-typed contracts across crate boundaries* — showing up a third time.

## Goals & Non-Goals

**Goals:**
- Reach a **decision** on the egress seam — and note whether the real action is the string-key coupling, not the
  allow-list.

**Non-Goals:**
- Building a per-connection allow-list speculatively. The signal to act is a *concrete* need for scoped egress
  (multi-tenant hardening) — absent that, don't deepen.

## Detailed Design

*Decision input.* Three honest directions: (a) **build the second adapter** — a real per-connection/-tenant
allow-list — making the seam real, *if* scoped egress is on the roadmap; (b) **leave `allow_all`** as the only policy
and let the trait stay a thin future-proofing seam; (c) the likely-higher-value move — fold the shared `auth_scheme`
vocabulary into one typed schema alongside [[WEIR-I-0033]], addressing the coupling rather than the seam.

## Alternatives Considered

- **Collapse `EgressPolicy`/`HostAllowList` to a concrete credential-injector.** Concentrates the auth logic, but the
  trait is a deliberate fidius extension point (sandboxed guests, plausible third-party policies) — removing it
  *moves* the extensibility cost rather than deleting it. Weigh in the decision.

## Implementation Plan

Single step — **decide**:
- [x] **Make a decision to promote for fix**: either (a) promote to a fix initiative (per-connection allow-list, or
  the typed auth-scheme schema with [[WEIR-I-0033]]), or (b) close, recording in an ADR that `allow_all` is
  intentional until scoped egress is needed — so future reviews don't re-suggest it.

## Decision (2026-08-17)

**Split decision, as the ticket itself anticipated.** (1) The **seam stays as-is**: `allow_all` remains the intentional production policy until scoped egress has a concrete driver (multi-tenant hardening beyond the alpha posture of [[WEIR-I-0046]]); do **not** build the per-connection allow-list speculatively. (2) The **real action — the shared `auth_scheme` vocabulary — merges into the promoted [[WEIR-I-0033]] refactor** (one typed schema, one writer, one reader). (3) New fact since filing: hostname-keyed TCP allow-lists are *impossible* today regardless of policy depth (`authorize_tcp` sees only the resolved `SocketAddr`; weir matches IP strings) — a **hostname-carrying TCP egress FR was filed with fidius on 2026-08-17** (`TcpTarget { host, addr }` + resolve-and-pin; see [[WEIR-A-0041]] Neutral consequences). When that lands, `HostAllowList` grows name matching, and any future per-connection allow-list becomes honest. This ticket's exit criterion is met.
