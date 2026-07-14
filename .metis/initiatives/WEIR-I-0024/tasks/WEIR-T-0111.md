---
id: per-tenant-health-dashboard-ui
level: task
title: "Per-tenant health dashboard (UI)"
short_code: "WEIR-T-0111"
created_at: 2026-07-07T04:06:02.262505+00:00
updated_at: 2026-07-07T11:05:48.057027+00:00
parent: WEIR-I-0024
blocked_by: [WEIR-T-0110]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0024
---

# Per-tenant health dashboard (UI)

## Parent Initiative

[[WEIR-I-0024]] — the "are my pipelines healthy?" screen.

## Objective

A tenant sees their systems **holistically**: a connection **status grid** (green/amber/red), a "needs
attention" list, and per-connection detail (last run + lag, throughput, dead letters). Reads at a glance.

## Reference

- `weir-ui/src/main.rs` — the Leptos app; the existing operations view + `areq_*` tenant-scoped fetch wrappers
  ([[WEIR-T-0095]]). The `/tenants/{id}/health` endpoint from [[WEIR-T-0110]].
- Aurora Dark styling already in the app; run-detail modal / lineage panel ([[WEIR-T-0101]]) for the detail shape.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] **Health dashboard** view (new "Health" nav segment): status grid of health cards (green/amber/red/unknown
  pill), worst-first, + a "Needs attention" table (amber/red, worst-first) on top.
- [x] Per-connection detail on each card: **lag** (freshness), error-rate, dead-letters, recent rows, + a
  **throughput sparkline** (SVG); the card drills to the run-detail modal.
- [x] Tenant-scoped via `apath` → `GET /overview` (polled with runs); empty state ("add one in Setup").
- [x] e2e `health.spec.ts`: the dashboard renders the seeded connection's health card + both panels. UI e2e 8/8;
  UI builds clean (trunk). *(Note: e2e asserts the dashboard + card render; forcing a live red state needs a
  failing seed — the needs-attention filter itself is unit-covered in T-0110.)*

## Status Updates

### 2026-07-07 — done (`e17b72f`)

New **Health** view in `weir-ui`: `ConnHealth` type + poll of `GET /overview`; a "Needs attention" table
(amber/red, worst-first) + a status grid of `weir-health-card`s (health-coloured Aurora `Pill`, lag/error/
dead-letter stats, an SVG throughput sparkline), cards drill to the run modal. `health_color` (green→OK /
amber→GOLD / red→BAD / unknown→MUTED), `health_rank`, `fmt_lag`, `spark_points` helpers + card CSS. UI builds
clean; **e2e 8/8** incl `health.spec.ts`. **Complete.**
