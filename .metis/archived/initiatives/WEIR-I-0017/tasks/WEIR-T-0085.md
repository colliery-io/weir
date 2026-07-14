---
id: authorizer-seam-permissive-default
level: task
title: "Authorizer seam (permissive default) + audit events on mutations"
short_code: "WEIR-T-0085"
created_at: 2026-07-05T21:11:05.250154+00:00
updated_at: 2026-07-05T22:02:59.163792+00:00
parent: WEIR-I-0017
blocked_by: [WEIR-T-0084]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0017
---

# Authorizer seam (permissive default) + audit events on mutations

## Parent Initiative

[[WEIR-I-0017]]. The periphery-attach seam + the audit trail. Governed by [[WEIR-A-0008]] / [[WEIR-A-0005]].

## Objective

Add the **`Authorizer` seam** — invoked on every endpoint with `(Principal, Action, Resource)` — shipping a
**`Permissive`** core implementation (any authenticated principal allowed). Emit an **`AuditEvent` on every
mutation** to the `audit_events` table + structured log. This is the interface the periphery's RBAC engine
replaces without touching handlers.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **`Authorizer` trait** — `authorize(&self, principal: &Principal, action: Action, resource: &Resource)
  -> Result<(), Denied>`; `Action` (enum: e.g. `ConnectionCreate/Run/Delete`, `CatalogImport`, `TokenMint`,
  `Read`) + `Resource` (typed: connection name / catalog entry / global). A **`Permissive`** impl returns
  `Ok(())` for any authenticated principal; a `Denied` → **403**.
- [ ] **Every endpoint calls it** — a route→(action,resource) mapping so each handler (or one layer) invokes
  `authorize` after AuthN. The `Authorizer` is injected (app state / extension) so the periphery swaps it.
- [ ] **Audit on mutations** — every state-changing endpoint (connection create/run/delete, catalog
  import/unregister, token mint/revoke) writes `AuditEvent { actor: principal.subject, action, resource, ts,
  outcome: ok|denied|error }` to `audit_events` + a structured log line. Reads are **not** audited.
- [ ] A denied action (using a deliberately-denying test `Authorizer`) → 403 **and** an `outcome=denied`
  audit row — proving the seam + audit both fire.
- [ ] `GET /audit` (or similar) lists recent audit events (authenticated) — optional but useful for the UI/tests.
- [ ] clippy clean; existing suites green.

## Technical Notes

- Keep `Authorizer` object-safe (`dyn Authorizer` in app state) so the periphery can drop in an RBAC impl.
- Map routes to actions in one place (a small table/fn) rather than scattering `authorize` calls ad hoc where
  it risks being forgotten — a missing check is a security hole.
- Audit write failures must not silently drop — log loudly; a mutation that can't be audited should fail.

## Dependencies

- **Blocked by [[WEIR-T-0084]]** (needs the `Principal` from AuthN). Uses the `audit_events` table from [[WEIR-T-0083]].

## Status Updates

### 2026-07-05 — implemented as the cloacina default-deny route table (not "permissive")

Per the [[WEIR-A-0008]] realignment, built the cloacina ABAC, **not** the permissive Authorizer this AC
predates:
- **`weir-api/src/authz.rs`** — `Level {Read<Write<Admin}` (`from_permissions`, fail-safe) · `Scope
  {Platform, Any}` · `Access` · `Principal {role, platform_admin}` (`from_key`) · `evaluate` (total,
  default-deny: god-mode passes, `Platform`=admin-only, `Any` needs `role≥level`) · **`build_authz_table()`**
  classifying every route (`(Method, matched-path) → Access`; GET=Read, mutations=Write). Mirrors cloacina
  `routes/authz.rs`. (weir has no tenant-scoped routes yet → all `Scope::Any` for now.)
- **`authz_mw`** wired after AuthN (`route_layer` order: authn outer → authz inner): matched-path lookup →
  **fail-closed 403** on any unclassified route → `evaluate` → 403 or continue.
- **Audit** — `App::record_audit(actor, action, resource, outcome)` + `recent_audit` (weir-app auth.rs); every
  **mutation** writes an `audit_events` row (`ok`|`denied`|`error`) with the key actor.

**Verified:** `authz_read_key_denied_write_and_audited` — a `read` key GETs 200 but POSTs 403, and the denial
lands in the audit trail; the 7 existing api tests still pass (admin bootstrap key clears authz). clippy clean.
**Complete.**
