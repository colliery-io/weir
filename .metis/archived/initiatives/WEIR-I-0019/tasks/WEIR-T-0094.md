---
id: admin-cross-tenant-data-routes
level: task
title: "Admin cross-tenant data routes /tenants/{id}/*"
short_code: "WEIR-T-0094"
created_at: 2026-07-06T01:23:41.351077+00:00
updated_at: 2026-07-06T01:28:46.054674+00:00
parent: WEIR-I-0019
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0019
---

# Admin cross-tenant data routes /tenants/{id}/*

## Parent Initiative

[[WEIR-I-0019]] — the backend the UI switcher rides. Governed by [[WEIR-A-0036]] d3 (explicit `/tenants/{id}/*`).

## Objective

Let a **platform-admin** read/act on **any tenant's** operational data via explicit `/tenants/{id}/…` routes
(the tenant from the path), reusing the tenant-scoped `weir-app` methods. This is what the UI switcher targets
when an admin selects a tenant.

## Reference

- `crates/weir-api/src/lib.rs` — the data handlers already take `tenant_of(key)`; the store methods already take
  `tenant` ([[WEIR-T-0090]]). `authz.rs` — `Scope::Platform` + `build_authz_table` ([[WEIR-T-0085]]/[[WEIR-T-0090]]).
- The `/tenants/*` admin surface (tenants + keys) already exists ([[WEIR-T-0090]]).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `/tenants/{id}/…` mirrors the read/act data API: `GET connections`, `GET connections/{name}`,
  `POST connections/{name}/run`, `GET .../runs`, `GET .../dead-letters`, `GET .../logs`, `GET .../state`,
  `GET catalog`, `GET runs` (+ `POST connections`, `DELETE connections/{name}` for full parity if cheap).
- [ ] The tenant comes from the **path**; these routes are **platform-admin only** (`Scope::Platform` in
  `authz.rs`) — a non-admin key gets **403**.
- [ ] Implemented via a **`TenantCtx` extractor** (path tenant when present + is_admin, else `tenant_of(key)`)
  so the existing handlers serve both mounts — or thin wrapper handlers if the dual-mount proves fiddly.
- [ ] Mutations audit with the target tenant + the acting admin key.
- [ ] api tests: an admin reads/acts on tenant X via `/tenants/X/…`; a non-admin key → 403; the implicit routes
  are unchanged. Workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`0d419e6`)

Chose **thin wrapper handlers** over the `TenantCtx` dual-mount extractor — simpler + explicit, and authz
already gates them so the wrappers just take the path tenant + call the same `App` method. Added
`t_connections`/`t_connection`/`t_run`/`t_runs`/`t_dead_letters`/`t_logs`/`t_state`/`t_catalog`/`t_recent`
under `/tenants/{id}/…` (weir-api/src/lib.rs), the 9 routes, and the 9 `Scope::Platform` authz entries
(authz.rs). Mutations (`.../run`) audit via the existing `authz_mw` (actor = admin key, resource carries the
tenant). Kept it read/act browse (no create/delete cross-tenant — the switcher browses; a tenant self-serves
via the implicit routes).

Test **`admin_cross_tenant_browse`**: admin sees acme's `c1` via `/tenants/acme/connections`; admin's own
implicit list stays empty (no leak); a **non-admin key → 403** on the cross-tenant route. api suite **12/12**;
clippy clean. **Complete.**
