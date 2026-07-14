---
id: ui-tenants-admin-panel-tenants-keys
level: task
title: "UI: tenants admin panel (tenants + keys)"
short_code: "WEIR-T-0096"
created_at: 2026-07-06T01:23:48.573994+00:00
updated_at: 2026-07-06T01:40:09.746198+00:00
parent: WEIR-I-0019
blocked_by: [WEIR-T-0094]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0019
---

# UI: tenants admin panel (tenants + keys)

## Parent Initiative

[[WEIR-I-0019]]. Drives the existing `/tenants/*` + `/tenants/{id}/keys` routes ([[WEIR-T-0090]]).

## Objective

A **platform-admin** panel to administer tenants: list/create tenants, and list/mint/revoke each tenant's API
keys. Gated on `is_admin` (hidden for non-admins).

## Reference

- Routes ([[WEIR-T-0090]]): `GET/POST /tenants`, `DELETE /tenants/{id}`, `GET/POST /tenants/{id}/keys`,
  `DELETE /tenants/{id}/keys/{kid}`. `ApiKeyDto` returns metadata (never the secret); create returns the
  plaintext **once**.
- Aurora `Panel`/`Table`/`Modal`/`Button` for the surface; the auth gate ([[WEIR-T-0087]]) for `is_admin`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A "Tenants" admin view (nav entry visible only when `is_admin`): a table of tenants (id/name/created) +
  create.
- [ ] Per-tenant: list its keys (metadata), **mint** a key (show the plaintext once, in a modal, with a copy),
  **revoke** a key.
- [ ] Non-admins never see the view; the routes 403 for them anyway (defence in depth).
- [ ] wasm builds; `angreal ui build`; clippy clean.

## Status Updates

### 2026-07-06 — done (`d19aae8`)

A **header "tenants" link** (rendered only when `is_admin`) opens an **overlay** (mirrors the auth-gate overlay
pattern — cleaner than surgery on the big Operations/Setup view-match). The overlay:
- **Create tenant** (id input → `POST /tenants`) + a **table** of tenants (`GET /tenants`), each with a "keys →".
- Selecting a tenant loads its keys (`GET /tenants/{id}/keys`) → a table with **revoke** (`DELETE …/keys/{kid}`);
  **mint** (name → `POST …/keys`) shows the plaintext **once** in a copy-now banner.
- Non-admins never see the link; the routes 403 anyway (defence in depth).

Signals: `show_tenants`/`sel_tenant`/`tenant_keys`/`minted_key` + input signals; closures `reload_tenants`/
`load_keys`/`create_tenant`/`mint_key`/`revoke_key` (all `/tenants/*`, never re-scoped by `apath`). wasm `cargo
check` + clippy clean. The trunk embed + behaviour check ride with the e2e ([[WEIR-T-0097]]). **Complete.**
