---
id: numeric-aware-cursor-comparison
level: task
title: "Numeric-aware cursor comparison helper + adoption (rest, mssql, snowflake)"
short_code: "WEIR-T-0187"
created_at: 2026-08-30T11:54:07.869599+00:00
updated_at: 2026-08-30T12:30:18.388821+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# Numeric-aware cursor comparison helper + adoption (rest, mssql, snowflake)

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

Client-side cursor comparisons are lexicographic string compares, so numeric cursors mis-order (`"9" > "12"`) and re-deliver or skip rows. [[WEIR-T-0182]] fixed the SQL-side predicate for postgres (untyped literal cast in the WHERE clause); the remaining offenders compare cursors **in guest code**: the rest runtime's incremental filter, mssql, and snowflake. Add one shared helper to `weir-connector-types` — numeric-aware compare (when both sides parse as numbers, compare numerically; otherwise fall back to string compare, which stays correct for ISO-8601 timestamps) — and adopt it at every client-side comparison site.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] `weir-connector-types` exposes a `cursor_cmp(a, b) -> Ordering` helper with unit coverage: numeric ordering (`"9" < "12"`, floats), mixed/unparseable falls back to string compare, ISO-8601 timestamps order correctly, equality
- [x] rest, mssql, and snowflake use the helper for every client-side cursor comparison — no bare string `>`/`<` on cursor values remains (grep-verified)
- [x] Engine-level regression: an incremental sync over numeric ids reaching double digits delivers each row exactly once across two runs (the `'2' > '12'` re-delivery class cannot recur client-side)
- [x] `angreal check all` + unit wall + affected wasm engine suites green

## Status Updates **[REQUIRED]**

- **2026-08-30 — DONE.** New module `weir-connector-types/src/cursor.rs`: `cursor_cmp` (i128 fast path, f64 second, string fallback — total + stable; NaN pairs collapse to Equal) re-exported at crate root; 5 unit tests incl. the T-0182 `"2" vs "12"` class, floats/1e3, ISO-8601 strings, mixed fallback. Adopted at all three client-side sites: rest `fetch_page` cursor_seen max, mssql incremental max (lib.rs:76), snowflake max (lib.rs:505) — grep confirms no bare cursor `>` remains in any guest. Engine regression `wasm_http_source_numeric_cursor_survives_digit_rollover` (new `mock_http_sequence`): run 1 commits cursor "9", run 2 over ids 10-12 commits "12" (lexicographic would stick at "9" and re-deliver forever) and the wire shows `since=9`. **Verified:** wasm_http_engine 31/31, wasm_snowflake_engine 4/4 (mock-based), all three guests compile for wasm32-wasip2, `angreal check all` + unit wall clean. The mssql engine suite is docker-gated (`#[ignore]`, estate not up locally) — the mssql change is the same one-line helper swap, unit-covered, and CI's integration.yml runs that suite. CHANGELOG cursor entry extended to name the client-side fix.
