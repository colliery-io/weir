---
id: runs-feed-pagination-get-single-run
level: task
title: "Runs feed pagination + GET single run"
short_code: "WEIR-T-0189"
created_at: 2026-08-30T11:54:16.637310+00:00
updated_at: 2026-08-30T13:06:54.571633+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# Runs feed pagination + GET single run

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

The runs feed (`run_feed`) returns a fixed recent window with no way to page further back, and there is no endpoint to fetch a single run by id — so once [[WEIR-T-0188]] retention is the only bound, an operator still can't walk history or deep-link a specific run from the UI. Add cursor/limit pagination to the runs feed and a `GET` for one run (with its log tail), tenant-scoped like the rest of the control-plane API.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] Runs feed accepts `limit` (bounded, sane default) + a stable cursor (e.g. before-id/created_at) and returns the next-page cursor; ordering is newest-first and stable across pages (no dupes/gaps at page boundaries)
- [x] `GET /…/runs/{id}` returns the single run — status, timings, counts, error, log tail — 404 on unknown id, 404/403 across tenants (no cross-tenant leak)
- [x] Both endpoints work on SQLite and Postgres backends; API docs/openapi surface updated
- [x] Tests: pagination walks a >2-page history exactly once, single-run fetch happy path + unknown id + cross-tenant denial
- [x] `angreal check all` + unit wall + functional suite green

## Status Updates **[REQUIRED]**

- **2026-08-30 — DONE.** Design choice on the cursor: the response stays `Vec<RunRow>` (no envelope — the UI's existing `/runs` consumer is untouched); the next-page cursor is **derivable as the smallest `id` of the page** (work-unit ids are monotonic), documented in the handler doc + docs/api/index.md. `?before` absent = the LIVE first page (recent window ∪ active union, preserving the resident-visibility guarantee from I-0035); `?before=<id>` = strict id-descending history with no active union, so the walk is dupe/gap-free.
  - **weir-app**: `store::run_feed_before` (pure `id < before` window) + `store::run_by_id` (tenant+id → `RunDetailRow` with `stream`); `App::recent_runs_before(tenant, limit, before)` (recent_runs delegates), `App::run_detail` → new `RunDetail` (feed fields + stream + raw timestamps + duration + 50-line run-log tail; logs are connection-scoped, noted in the type). Cross-tenant = `None` by the (tenant_id, id) filter.
  - **weir-api**: `RunsQuery {limit (default 50, clamp 1..=500), before}` on `GET /runs` and admin `GET /tenants/{id}/runs`; new `GET /runs/{id}` (Access::any(Read)) + admin `GET /tenants/{id}/runs/{run_id}` (platform Admin) — authz table entries added; 404 on unknown/foreign id.
  - Plain portable diesel on both dualdb backends by construction. docs/api/index.md ops row updated; CHANGELOG Added entry.
  - **Tests**: weir-app `runs_feed_tests` — `cursor_walk_covers_history_exactly_once` (25 terminal + 1 old running + 1 foreign-tenant unit; live page = 10 newest ∪ active, then a 3-page walk covering ids 15..0 exactly once in order) and `run_detail_scopes_by_tenant` (happy path incl. stream, unknown id None, other-tenant None); weir-api api.rs extended — `GET /runs/{boom_id}` 200 with state/connection/logs, unknown id 404, `?limit=1` caps the page. weir-app 31 lib + all integration, weir-api 16, functional, unit wall, `angreal check all` — all green.
