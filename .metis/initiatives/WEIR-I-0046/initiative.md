---
id: multi-tenant-posture-for-alpha-de
level: initiative
title: "Multi-tenant posture for alpha — de-advertise, fix the safety class"
short_code: "WEIR-I-0046"
created_at: 2026-08-18T01:57:43.432520+00:00
updated_at: 2026-08-18T02:04:34.262100+00:00
parent: WEIR-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/design"


exit_criteria_met: false
estimated_complexity: S
initiative_id: multi-tenant-posture-for-alpha-de
---

# Multi-tenant posture for alpha — de-advertise, fix the safety class Initiative

## Context **[REQUIRED]**

The API sells tenancy; several seams don't deliver it (2026-08-16 review): App::stop ignores its tenant argument and Relay::cancel/has_active filter work_units by connection name only (cross-tenant stop hole — `crates/weir-app/src/lib.rs:392-394`, `crates/weir-orchestrator/src/lib.rs:967-1011`); OIDC mints every IdP-authenticated user a write key on the default tenant (`crates/weir-api/src/oidc.rs:322-330`); delete_tenant orphans everything and keys of deleted tenants keep validating (`crates/weir-app/src/tenant.rs:84-97`, `auth.rs:133-174`); admin-supplied tenant ids are used raw in filesystem joins (`../x` escapes the connectors dir — `weir-app/src/lib.rs:118-136`, `ingress.rs:61-63`); discover is SSRF-shaped at Read level; catalog import runs `cargo build` on the control-plane host for any tenant Write key.

Per [[WEIR-A-0004]], the managed multi-tenant experience is periphery — the open core owes isolation *primitives*, not a hardened multi-tenant product. The alpha audience is a single operator. **The posture decision this initiative encodes: de-advertise multi-tenancy for the alpha ("tenants are workspaces for trusted operators"), while fixing the safety-class holes that a public alpha binary must not ship regardless of posture.**

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- The safety bundle: tenant ids validated to a safe slug charset; cancel/has_active/stop scoped by the [[WEIR-A-0036]] composite (tenant_id, name) identity; tenant deletion either cascades or refuses-if-nonempty; keys of deleted tenants stop validating.
- The posture is documented loudly: tenants are trusted-operator workspaces in alpha; OIDC's default-tenant write-key behavior stated; catalog crate-import's trust boundary stated.
- The OIDC identity→tenant/role mapping is *designed* (decision task, ADR-worthy) for post-alpha build.

**Non-Goals:**
- Full multi-tenant hardening: OIDC mapping implementation, discover SSRF gating (move to Write + tenant scope), catalog-import build isolation, autoscaler leased-resident awareness, read auditing — all post-alpha, seeded from this initiative's decision task.
- All-tenant scheduling — decided/characterized in [[WEIR-T-0171]].
- Per-connection egress allow-lists — [[WEIR-I-0034]] decision stands (not until scoped egress has a driver).

## Detailed Design **[REQUIRED]**

The composite-identity convention is already decided ([[WEIR-A-0036]]: (tenant_id, name) keys, implicit default tenant) — the safety bundle *applies* it to the seams that ignore it; no new architecture. Tenant-id validation is a constraint at creation (admin-only surface, so a strict slug — `[a-z0-9][a-z0-9-]{0,62}` — breaks nobody real). Delete semantics: recommend refuse-if-nonempty for alpha (cheapest honest behavior; cascade is a follow-up with the retention machinery of [[WEIR-I-0044]]).

### Candidate decomposition

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | Safety bundle: tenant-id slug validation; tenant-scoped cancel/has_active/stop; delete refuse-if-nonempty; deleted-tenant key invalidation; regression tests incl. a cross-tenant stop attempt | week | the must-ship |
| 2 | Posture documentation: trusted-operator workspaces, OIDC default-tenant behavior, crate-import trust boundary | days | docs + a startup log line |
| 3 | Decision task: OIDC identity→tenant/role mapping design (claims mapping? allowlist? per-tenant IdP?) → ADR | days | post-alpha build seeds from it |

## Testing Strategy

The safety bundle's tests are the deliverable's proof: cross-tenant stop attempt (Write key of tenant A vs same-named connection of tenant B) is denied and audited; `../x` tenant id rejected at creation; deleted tenant's key 401s within the cache TTL; delete of a non-empty tenant refuses with a clear message.

## Alternatives Considered **[REQUIRED]**

- **Harden fully before alpha** — rejected: weeks of work for periphery-classified capability ([[WEIR-A-0004]]) the alpha audience doesn't exercise; it would displace the initiatives that gate the actual alpha promise.
- **Remove/flag-gate tenancy for alpha** — rejected: the tenancy surface is load-bearing for existing tests, the demo estate, and the per-tenant runner model ([[WEIR-A-0036]]); de-advertising with a documented posture is cheaper and reversible.
- **Ship as-is with docs only (no safety bundle)** — rejected: path-traversal ids, cross-tenant cancel, and immortal deleted-tenant keys are exploitable-by-accident classes a public binary shouldn't carry, posture or not.

## Implementation Plan **[REQUIRED]**

Task 1 → 2 in sequence (docs state what the code now enforces); 3 any time. Alpha cut: 1 + 2. No dependencies on other initiatives; [[WEIR-T-0171]]'s schedule-tenancy decision should land first or together so the posture docs describe one coherent story.

## Exit Criteria

- [ ] The cross-tenant stop/cancel regression test passes; `../` tenant ids are rejected; deleted-tenant keys stop validating; non-empty tenant deletion refuses
- [ ] Published docs state the alpha tenancy posture in one place, linked from the tenants guide
- [ ] The OIDC mapping ADR draft exists with a recommended design
