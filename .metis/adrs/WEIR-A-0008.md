---
id: 001-auth-baseline-rbac-seam
level: adr
title: "Auth baseline & RBAC seam"
number: 8
short_code: "WEIR-A-0008"
created_at: 2026-06-17T02:11:58.497351+00:00
updated_at: 2026-07-05T00:00:00.000000+00:00
decision_date: 2026-07-05
decision_maker: dylan.storey@gmail.com
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: true
initiative_id: NULL
---

# ADR-0008: Auth baseline & RBAC seam

**Status:** Decided (revised 2026-07-05 to mirror Colliery **cloacina**'s proven auth model — see
"Prior art"). *Raised by: [[WEIR-S-0002]]. Realized by [[WEIR-I-0017]].*

## Context

The control-plane API is currently **wide open**. weir adopts cloacina's auth architecture wholesale — it is
proven + working in a sibling Colliery control plane, and re-using it (rather than a bespoke minimal seam)
gives weir a coherent, hardened baseline. This **revises** the original draft (which parked tenancy/RBAC at
the periphery) and, with it, **decides [[WEIR-A-0004]]** (multi-tenancy): the core is multi-tenant ABAC.

## Decision

**One auth primitive (the API key), tenant+role ABAC in the core, a default-deny route table, and OIDC as a
key-minter.** Mirrors `cloacina-server`.

1. **Keys are the only primitive.** A key is `weirk_<random>`; the store holds its **SHA-256 hash** (keys are
   high-entropy — no argon2/salt needed, and a plain hash enables O(1) lookup + caching). `api_keys` columns:
   `id, name, key_hash, tenant_id (nullable), role (read|write|admin), is_admin (bool), created_at,
   last_used_at, expires_at (nullable), revoked_at (nullable)`.
2. **AuthN middleware with an LRU cache.** Extract `Authorization: Bearer <key>` → SHA-256 → check an
   **LRU cache (30s TTL)** → else DAL `validate_hash` → insert an `AuthenticatedKey { key_id, name, role,
   tenant_id, is_admin }` into request extensions; else **401**. Applied via `route_layer` so `/health` + the
   SPA fallback stay public.
3. **AuthZ = a total, default-deny route table.** `Level {Read,Write,Admin}` (`from_permissions`, fail-safe
   to Read) · `Scope {Platform, TenantParam, Any}` → `ResolvedScope {Platform, Tenant(id), Any}` · `Access
   {scope, level}` · `Principal {tenant, role, platform_admin}` (`from_key`) · `Decision {Permit,
   Deny(reason)}` · `evaluate(principal, scope, level)` (god-mode short-circuits; `Platform`=admin-only;
   `Tenant(t)` requires principal∈t then role≥level; `Any` requires role≥level). A **`build_authz_table()`**
   maps every `(Method, path)` → `Access`; **a route absent from the table is denied** (fail-closed) — the
   critical property.
4. **OIDC mints a short-lived key.** Login (generic OIDC, Dex in test) resolves a `ResolvedPrincipal`
   (subject/tenant/role); `mint_for_principal` mints an **ephemeral (~15-min) key** carrying that tenant+role;
   an httpOnly cookie holds it. **No separate session table** — a session *is* a short-lived key, so the whole
   system has one validation path.
5. **Audit** — every mutation emits an `AuditEvent` (actor, action, resource, ts, outcome) to the store + log.
6. **UI** — a `RequireAuth` gate: unauthenticated → sign-in (OIDC) or API-key entry; authenticated views ride
   the cookie/key.

**Core vs periphery ([[WEIR-A-0005]]):** *core* = keys + the ABAC table + one generic OIDC provider + local
audit + tenant CRUD. *Periphery* = enterprise SSO (multi-IdP, group→role, SCIM), audit forwarding, advanced
policy.

## Prior art

Mirrors `~/Desktop/cloacina` (`cloacina-server`): `security/api_keys.rs` (`hash_api_key` = SHA-256,
`generate_api_key`), `routes/auth.rs` (LRU `KeyCache` + `validate_token`), `routes/authz.rs`
(`Level`/`Scope`/`Access`/`Principal`/`evaluate`/`build_authz_table`), `identity.rs` (`ResolvedPrincipal`,
`mint_for_principal`), `routes/{oidc_auth,session,local_auth}.rs`, `security/audit.rs`, `ui/.../RequireAuth`.

## Alternatives Analysis

| Option | Pros | Cons |
|--------|------|------|
| **Mirror cloacina (chosen)** | Proven, coherent, hardened; one validation path; default-deny; tenants+roles first-class | More surface than a minimal seam; commits weir to multi-tenant now |
| Bespoke minimal seam (prior draft: argon2 keys + permissive authz + separate sessions) | Smaller | Reinvents a solved problem; allow-all authz is a footgun; two validation paths |
| Full RBAC + SSO in core | — | Pushes periphery value into core ([[WEIR-A-0005]]) |

## Consequences

### Positive
- Proven design; a route you forget to classify is **denied**, not open. One key-validation path (fast + cached).
- Tenants + roles are first-class from day one; OIDC and keys unify.

### Negative
- Larger core (ABAC table, OIDC, tenant CRUD, audit); **reworks the in-flight [[WEIR-T-0083]]** (argon2/prefix
  + a `sessions` table → SHA-256 + tenant/role columns, no sessions table).
- Commits weir to a tenant model now (decides [[WEIR-A-0004]]).

### Neutral
- Rides the diesel-dualdb store; bounded by [[WEIR-A-0005]]/[[WEIR-A-0006]]; distinct from connector creds
  ([[WEIR-A-0013]]/[[WEIR-A-0033]]).

## Relationships

- **Realized by** [[WEIR-I-0017]] (rescoped to the cloacina model).
- **Decides** [[WEIR-A-0004]] (multi-tenancy — the core carries `tenant_id` + tenant-scoped ABAC).
- Bounded by [[WEIR-A-0005]]; client of [[WEIR-A-0006]]; UI gate per [[WEIR-A-0024]].
