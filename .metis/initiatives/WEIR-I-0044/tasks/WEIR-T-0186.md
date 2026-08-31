---
id: status-aware-end-of-data-error
level: task
title: "Status-aware end-of-data — error pages fail the run, never success-with-partial-data"
short_code: "WEIR-T-0186"
created_at: 2026-08-30T11:54:03.056194+00:00
updated_at: 2026-08-30T12:18:50.851996+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# Status-aware end-of-data — error pages fail the run, never success-with-partial-data

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

The worst honesty bug in the rest runtime: a non-2xx or non-JSON response past page 1 is treated as normal end-of-data, so a mid-sync auth expiry or rate-limit ends the run "successfully" with partial data. Make stop conditions status-code-aware: 2xx-with-empty-records = genuine end; 4xx/5xx (or unparseable body) at ANY page = the run FAILS as an error carrying the status + page position — safe, because with [[WEIR-T-0184]]'s streaming checkpoints all prior progress is already committed and the retry resumes from it.

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] A 4xx/5xx on any page surfaces as `ReadMessage::Fatal` (transient for 429/5xx, so retries fire; fatal for 4xx auth/config classes) naming status + page — never a clean end
- [x] A 2xx page with an empty/absent record array still ends the read cleanly (the legitimate end-of-data)
- [x] Mock-HTTP engine test: pages 1-2 OK then a 401 → run FAILS, the checkpoint through page 2 is committed, a re-run resumes at page 3 (lands with [[WEIR-T-0184]])
- [x] `angreal check all` + unit wall + wasm_http_engine + manifest corpus green

## Status Updates **[REQUIRED]**

- **2026-08-30 — Activated; plan.** Add a status gate in `fetch_page` (rest/src/lib.rs) right after `send_with_retry`, before body parsing: non-2xx → error, with two carve-outs: (1) **404 past page 1 = clean end** — page-probing APIs 404 one-past-the-end, and the same URL shape already succeeded so it can't be a config error; (2) **401 = transient** (not fatal) naming status+page — an expired host-side credential heals via the worker's run-level retry with a freshly resolved credential, resuming from the last committed page (T-0184). All other non-2xx (403/400/3xx/…) = fatal naming status + page. 429/5xx already surface as transient from send_with_retry after retries. On 2xx, tighten the old any-parse-failure-past-page-1=end heuristic to: EMPTY body past page 1 = end; non-empty unparseable = fatal at any page. Missing record_path / non-array past page 1 on 2xx stays end-of-data (APIs that 200 their error objects); 2xx empty record array stays the legitimate end. Tests: parameterize `mock_http_paged` with a sick-status code → (a) 401-at-page-3 test: run 1 FAILS (message names 401 + page 3), outbox = 2 committed checkpoints, healed run 2 resumes at page 3 and ends on the 2xx-empty page 5 (config without page_size, proving empty-array end), wire-asserted request sequence; (b) 404-past-end test: never-healed 404 at page 3 = clean end with 4 rows.
- **2026-08-30 — DONE.** Status gate landed in `fetch_page` exactly per plan (rest/src/lib.rs): non-2xx → error naming status + page (401 transient for the credential-heal path, 404-past-page-1 the sanctioned clean end); 2xx unparseable-body now fatal at any page (empty-body-past-page-1 still ends). `mock_http_paged` parameterized with a sick-status code. **Verified:** `wasm_http_source_fails_run_on_mid_pagination_error_page` (401 at page 3 fails run 1 with "401"+"page 3" in the error, outbox=2, healed run 2 resumes at page 3 and ends on the 2xx-empty page 5, requests wire-asserted `[1,2,3,3,4,5]`) and `wasm_http_source_treats_404_past_last_page_as_end` (4 rows, clean) both pass; full wasm_http_engine 29/29, manifest corpus 36/36, `angreal check all` + unit wall clean. CHANGELOG Fixed entry added.
