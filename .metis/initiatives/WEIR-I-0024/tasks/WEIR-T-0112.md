---
id: super-operator-cross-tenant-view-ui
level: task
title: "Super-operator cross-tenant view (UI)"
short_code: "WEIR-T-0112"
created_at: 2026-07-07T04:06:03.326429+00:00
updated_at: 2026-07-07T11:12:39.129593+00:00
parent: WEIR-I-0024
blocked_by: [WEIR-T-0110]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0024
---

# Super-operator cross-tenant view (UI)

## Parent Initiative

[[WEIR-I-0024]] — the "is the platform OK?" screen. Closes the initiative.

## Objective

The platform admin sees **the whole system**: per-tenant health rollups, what needs attention **across all
tenants**, and runner/fleet status — drill into a tenant → its connections (the [[WEIR-T-0111]] dashboard).

## Reference

- The `GET /health/overview` platform-admin endpoint from [[WEIR-T-0110]].
- `weir-ui/src/main.rs` — `is_admin` + the admin tenant switcher/overlay ([[WEIR-T-0095]]/[[WEIR-T-0096]]); gate
  this view the same way. Reuse the per-connection health components from [[WEIR-T-0111]].

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Admin-only Platform view**: per-tenant health cards (rolled-up green/amber/red pill + conns/failing/
  dead-letters/queue-depth), worst-first.
- [x] Cross-tenant **"needs attention"** table (failing connections across all tenants) + **fleet** stats
  (tenants / active / total queue depth).
- [x] **Drill-in**: click a tenant card/row → re-scopes the active tenant + switches to the Health view
  ([[WEIR-T-0111]], `/overview` → `/tenants/{id}/overview` via `apath`).
- [x] The Platform nav segment is shown **only for `is_admin`**; the endpoint 403s non-admins ([[WEIR-T-0110]]).
- [x] e2e: admin sees `default` in the rollup. UI e2e **9/9**; UI builds clean. *(non-admin-can't is API-tested
  in T-0110; the UI simply hides the segment.)*

## Status Updates

### 2026-07-07 — done (`bd376fd`), closes I-0024

New **Platform** view (admin-only nav segment): `PlatformHealth`/`TenantHealth`/`AttentionItem` types + a poll of
`GET /platform/health` (guarded by `is_admin`; `apath` skips `/platform`). Renders fleet stats + a worst-first
grid of tenant health cards + a cross-tenant needs-attention table; `drill_tenant` re-scopes + jumps to the
Health dashboard. **e2e 9/9** incl `platform:` (admin sees the default tenant). **Complete.**
