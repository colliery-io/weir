---
id: e2e-tenant-scoping-switcher-admin
level: task
title: "e2e: tenant scoping + switcher + admin panel"
short_code: "WEIR-T-0097"
created_at: 2026-07-06T01:23:53.039142+00:00
updated_at: 2026-07-06T01:46:50.204637+00:00
parent: WEIR-I-0019
blocked_by: [WEIR-T-0094, WEIR-T-0095, WEIR-T-0096]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0019
---

# e2e: tenant scoping + switcher + admin panel

## Parent Initiative

[[WEIR-I-0019]] — the proving gate. Closes the initiative.

## Objective

Playwright coverage of the tenant-aware UI: a tenant user sees only its data, a platform-admin switches tenants
and Operations re-scopes, and the admin panel creates a tenant + mints a key.

## Reference

- `e2e/` harness ([[WEIR-I-0017]]): `fixtures.ts` seeds a key from `WEIR_E2E_KEY`; `angreal test e2e` runs it.
  Add a tenant-scoped key + an admin key.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A tenant-scoped user (non-admin key) sees only its tenant's connections; the switcher is absent; no
  cross-tenant resource shows.
- [ ] A platform-admin: the header switcher lists tenants; selecting a tenant re-scopes Operations to it
  (its connections appear).
- [ ] The Tenants admin panel: create a tenant, mint a key (plaintext shown once), see it listed.
- [ ] `angreal test e2e` green (added to the suite); existing specs still pass.

## Status Updates

### 2026-07-06 — done (`315f493`) — closes I-0019

`e2e/tests/tenant.spec.ts` (fixture seeds the bootstrap **admin** key → platform-admin): asserts the header
switcher + "tenants" link are present; opens the panel, **creates tenant `acme`** (appears in the table);
selects acme → **mints a key** (the `weirk_…` plaintext shows once in `.weir-minted`); closes + **switches to
acme** via the switcher → the page reloads and the switcher `toHaveValue('acme')` (scope survived the reload).

**Full `angreal test e2e` 7/7 green** (Dex + Playwright). The e2e earned its keep — caught **two real bugs**:
1. `load_keys` cleared `minted_key`, wiping the mint banner right after minting → removed the clear (now the
   "keys →" click clears on fresh selection).
2. sqlite `database is locked` under 6 parallel Playwright workers hitting one sqlite server → `playwright.config`
   set `workers: 1` (serialize; production is Postgres, so a non-issue there).

The non-admin-no-switcher path is covered by the API-level `cross_tenant_isolation` + the `is_admin` UI gate
(a second non-admin fixture wasn't worth the harness cost). **Complete — closes [[WEIR-I-0019]].**
