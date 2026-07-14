---
id: auth-baseline-api-keys-oidc-login
level: initiative
title: "Auth baseline — API keys + OIDC login + authz seam + audit"
short_code: "WEIR-I-0017"
created_at: 2026-07-05T20:39:58.657537+00:00
updated_at: 2026-07-05T22:35:20.498649+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: auth-baseline-api-keys-oidc-login
---

# Auth baseline — API keys + OIDC login + authz seam + audit Initiative

## Context

The control-plane API is **wide open** — every route serves with no authentication. Per [[WEIR-A-0008]],
the open-core baseline is **two AuthN doors that resolve to one `Principal`** — **API keys** (bearer, hashed,
CLI-minted) for machines and **generic OIDC login** (httpOnly cookie session) for humans — plus a pluggable
**`Authorizer` seam** (permissive single-tenant default; RBAC attaches at the periphery per [[WEIR-A-0005]])
and an **audit event on every mutation**. Enterprise SSO/RBAC stays periphery. **Dex** is the OIDC provider
in the integration/test stack.

## Goals & Non-Goals

**Goals**
- Authenticate every endpoint (except `/health`); reject unauthenticated requests with 401.
- API keys: mint via CLI, store hashed (argon2), bootstrap an admin key at `init`.
- Generic OIDC login: Authorization-Code flow against a configured provider → httpOnly cookie session (CSRF).
- `Authorizer` seam invoked on every route (permissive core impl); `AuditEvent` on every mutation.
- The UI gates on auth (OIDC sign-in + API-key entry) and rides the session.
- Dex-backed integration test proving keys + OIDC both authenticate end-to-end.

**Non-Goals**
- RBAC policy engine, multiple IdPs, group→role mapping, SCIM (periphery).
- JWT issuance by the core; connector-credential auth (that's [[WEIR-A-0013]]/[[WEIR-A-0033]], separate).
- Rate limiting / WAF concerns.

## Current state (what to change)

- `crates/weir-api/src/lib.rs` — `Router::new()` wires all routes with **no `.layer()` auth**; `/health`,
  `/connections*`, `/catalog*`, `/connectors*`, `/runs`. Add an AuthN layer + route→action mapping.
- Control-plane schema — diesel-dualdb (`angreal schema gen` from logical DDL). Add `api_keys`, `sessions`,
  `audit_events`.
- `weir-cli` — add `weir auth token create`; `init` mints + prints the bootstrap admin key.
- `weir-ui` (Leptos) — an auth gate (sign-in / key entry) + send the session/bearer on API calls.

## Implementation Plan (decomposition)

- **[[WEIR-T-0083]] Schema + Principal + API keys + CLI/bootstrap:** `api_keys`/`sessions`/`audit_events`
  tables; `Principal` type; argon2 hash/verify; `weir auth token create`; bootstrap admin key at `init`.
- **[[WEIR-T-0084]] AuthN middleware (bearer keys):** an axum layer validating `Authorization: Bearer`
  → `Principal` (401 otherwise), applied to all routes except `/health`. Locks the API down for the
  machine path; existing tests/CLI updated to present the bootstrap key.
- **[[WEIR-T-0085]] Authorizer seam + audit:** `Authorizer` trait + `Permissive` impl; route→action map;
  `AuditEvent` emitted on every mutation to DB + structured log.
- **[[WEIR-T-0086]] Generic OIDC login + cookie session:** Authorization-Code flow (issuer/client config),
  callback → verify ID token → mint httpOnly CSRF-protected cookie session; middleware accepts the session
  (2nd door) → `Principal{kind:User}`.
- **[[WEIR-T-0087]] UI auth gate (Leptos):** on 401, show sign-in (OIDC redirect) + API-key entry; store +
  send credentials; logout; the existing views ride the authenticated session.
- **[[WEIR-T-0088]] Dex integration + e2e:** add Dex to the integration stack (`compose`); an integration
  test drives the OIDC flow against Dex end-to-end; assert keys + OIDC both authenticate + audit rows land.

T-0083 → T-0084 unblock the rest; OIDC (T-0086) + UI (T-0087) build on the middleware; Dex e2e (T-0088) closes it.

## Exit Criteria

- [x] Every endpoint except `/health` requires auth; unauthenticated → 401.
- [x] API keys mint/verify (**SHA-256**); `init` bootstraps + prints an admin key; `weir auth token create` works.
- [x] Generic OIDC login works against Dex → httpOnly cookie session; the UI signs in and drives the API.
- [x] Authz seam on every route (**default-deny route table**, not permissive); `AuditEvent` on every mutation.
- [x] Dex-backed e2e green (keys + OIDC); existing suites updated + green; clippy clean.

**Delivered (commits `c70ff6b`..`81ebcf8`, pushed):** all 6 tasks (T-0083..0088). weir's control plane is
authenticated (keys + OIDC), authorized (default-deny ABAC), and audited; both doors proven e2e against Dex;
a green `ui-e2e` CI workflow guards it. Follow-ups (CI wiring, replay tests, CSRF via SameSite=Lax) closed in
[[WEIR-T-0088]]. Deferred: tenant CRUD routes + a Rust tamper-token test.

## Realignment — 2026-07-05: mirrored cloacina (done)

Per the revised [[WEIR-A-0008]] (+ [[WEIR-A-0004]] decided), this initiative is **rescoped to mirror
`~/Desktop/cloacina`'s proven auth** — not the minimal seam the tasks below were written against. The plan +
task specs need updating; **[[WEIR-T-0083]]'s committed code (`09b5cc6`) needs rework**. Deltas:

- **Keys:** SHA-256 hash (not argon2/prefix); `api_keys` columns gain `tenant_id`, `role (read|write|admin)`,
  `is_admin`, `expires_at`; **drop the `sessions` table** (a session is a short-lived minted key). Mirror
  cloacina `security/api_keys.rs`. → reworks **T-0083**.
- **AuthN middleware:** LRU key cache (30s TTL) + `validate_token` → `AuthenticatedKey {key_id,name,role,
  tenant_id,is_admin}` extension. Mirror `routes/auth.rs`. → **T-0084** (in-flight, uncommitted — redo on this model).
- **AuthZ:** the default-deny **route table** — `Level`/`Scope`/`Access`/`Principal`/`evaluate` +
  `build_authz_table()` (unclassified route ⇒ denied). Mirror `routes/authz.rs`. → replaces T-0085's
  "permissive Authorizer".
- **OIDC:** `mint_for_principal` mints a ~15-min key (tenant+role); cookie holds it — **no session table**.
  Mirror `identity.rs` + `routes/{oidc_auth,session,local_auth}.rs`. → **T-0086**.
- **Audit:** mirror `security/audit.rs`. **UI:** `RequireAuth` gate (mirror `ui/.../RequireAuth`). → T-0085/T-0087.
- **New surface:** tenant CRUD + tenant-scoped key management (cloacina `/tenants/*`) — likely a new task.

**Status: DONE.** All deltas above were implemented + the whole initiative ralphed to completion against the
cloacina model — SHA-256 keys, LRU bearer middleware, default-deny authz table + audit, OIDC-mints-a-key +
cookie door, the Leptos `RequireAuth` gate, and Dex + the full e2e (both doors, 6/6 green in CI). The `sessions`
table was dropped as planned. Tenant CRUD (the "new surface") is deferred to a follow-on initiative — weir has
the tenant *model* (`tenant_id`/`Scope`/`evaluate`) but no tenant-scoped routes yet.
