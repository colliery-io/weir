---
id: tenant-aware-ui
level: initiative
title: "Tenant-aware UI"
short_code: "WEIR-I-0019"
created_at: 2026-07-05T23:45:33.417992+00:00
updated_at: 2026-07-06T01:46:53.527120+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: tenant-aware-ui
---

# Tenant-aware UI Initiative

## Context

Once the backend is tenant-isolated ([[WEIR-I-0018]]), the Leptos + Aurora UI ([[WEIR-I-0016]]) must **respect
tenancy**: a signed-in user sees + acts on only their tenant's connectors/connections/runs, a platform-admin
can switch/administer tenants, and the "signed in as …" context shows the active tenant. Today the UI is
tenant-blind — it lists everything and the API isn't yet tenant-scoped. This closes that gap on the UI side.

## Goals & Non-Goals

**Goals**
- **Active-tenant context** — after sign-in (`/auth/me` already returns `tenant`/`is_admin`), the UI shows the
  active tenant; a **platform-admin** gets a tenant **switcher** (scopes all views to the chosen tenant).
- **Scoped views** — Operations (cards/feed/run-detail) + Setup (catalog/onboard/connection form) show only the
  active tenant's resources; new connections/onboards are created under it. The `areq_*` calls already carry the
  bearer/cookie → the API scopes; the UI just reflects + selects tenant.
- **Tenant admin (platform-admin)** — a minimal tenants view: list/create tenants + manage a tenant's API keys
  (drives the [[WEIR-I-0018]] `/tenants/*` routes). Mirror cloacina-ui's tenant surfaces.
- **e2e** — extend the Playwright suite: a tenant-scoped user sees only its data; a cross-tenant resource is
  absent; the admin switcher changes scope.

**Non-Goals**
- The backend tenant scoping itself ([[WEIR-I-0018]] — this initiative consumes it).
- Enterprise SSO / group→tenant UI (periphery).
- A full RBAC admin console (role editing beyond read/write/admin) — later.

## Prior art (cloacina)

`~/Desktop/cloacina/ui/` (`cloacina-ui` chart + the React app): tenant context, tenant switcher, tenant-scoped
routes. weir's UI is Leptos ([[WEIR-I-0016]]) — mirror the *shape* (active tenant + switcher + scoped views),
not the React code. `/auth/me` ([[WEIR-T-0086]]) already returns tenant/is_admin.

## Weir surfaces to change

- `weir-ui/src/main.rs` — an active-tenant signal (from `/auth/me`); a header tenant chip + admin switcher;
  views read/scope by tenant; a tenants admin panel (Aurora `Panel`/`Table`).
- `e2e/` — tenant-scoping specs (fixtures already seed a key; add a tenant-scoped key).

## Design decisions (2026-07-06, approved)

**Full switcher** — a platform-admin can browse **any tenant's operational data**, not just manage tenants.
This spans **UI + a backend expansion**, and is consistent with [[WEIR-A-0036]] d3 (which endorsed explicit
`/tenants/{id}/*` for admins and only rejected *ambient* tenant).

1. **Backend — admin cross-tenant data routes.** Mirror the data API under `/tenants/{id}/…`
   (`connections`, `connections/{name}` + `/run`/`/runs`/`/dead-letters`/`/logs`/`/state`, `catalog`, `runs`),
   **platform-admin only** (`Scope::Platform`), tenant taken **from the path**, reusing the same `weir-app`
   methods (they already take `tenant`) + audit. Cleanest impl: a `TenantCtx` extractor that yields the path
   tenant when present (is_admin-gated) else `tenant_of(key)`, so one handler set serves both mounts — fall back
   to thin wrapper handlers if the dual-mount extractor proves fiddly.
2. **UI — implicit by default, explicit path when switched.** A non-admin key is one tenant (no switcher). A
   platform-admin gets a **header switcher** (lists `GET /tenants`); with no selection the UI uses the implicit
   data routes (its own `default` scope), and when a tenant is selected the `areq_*` calls target
   `/tenants/{id}/…`. The active tenant shows in the header.
3. **Tenants admin panel** (platform-admin) — list/create tenants + list/mint/revoke each tenant's keys via the
   existing `/tenants/*` + `/tenants/{id}/keys` ([[WEIR-T-0090]]).

## Proposed decomposition (for sign-off)

- **T-a (backend):** admin cross-tenant data routes `/tenants/{id}/…` + `TenantCtx` + authz `Scope::Platform` +
  audit; api tests (admin reads tenant X; a non-admin gets 403).
- **T-b (UI):** active-tenant context signal (from `/auth/me`) + header tenant chip + admin **switcher** that
  re-scopes `areq_*` to `/tenants/{id}/…`; Operations/Setup ride it.
- **T-c (UI):** tenants **admin panel** — list/create tenants + per-tenant key mint/revoke.
- **T-d (e2e):** a tenant user sees only its data; the admin switcher re-scopes Operations to another tenant; the
  admin panel creates a tenant + mints a key.

## Exit Criteria (draft — refine in design)

- [ ] The UI shows the active tenant; a platform-admin can switch tenants and all views re-scope.
- [ ] Operations + Setup show only the active tenant's resources; creates land under it.
- [ ] A platform-admin tenants panel lists/creates tenants + manages their keys.
- [ ] Playwright: a tenant user sees only its data; cross-tenant resources absent; switcher works. clippy clean.

## Dependencies

- **Blocked by [[WEIR-I-0018]]** (needs tenant-scoped API + `/tenants/*` routes).
