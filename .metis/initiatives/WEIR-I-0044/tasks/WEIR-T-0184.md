---
id: rest-runtime-streaming-checkpoints
level: task
title: "REST runtime streaming checkpoints — per-page-batch emission + paginator state in opaque"
short_code: "WEIR-T-0184"
created_at: 2026-08-30T11:53:54.886981+00:00
updated_at: 2026-08-30T12:13:49.057961+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# REST runtime streaming checkpoints — per-page-batch emission + paginator state in opaque

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

The headline durability fix: the declarative REST runtime (`crates/connectors/rest`) buffers an ENTIRE paginated read in memory and emits ONE checkpoint at the end — a failure at page 900 loses everything, memory is unbounded, and `MAX_PAGES=1000` silently truncates. Stream instead: emit `Records` + `Checkpoint` per page-batch, carrying the paginator's resume position in `StreamState.opaque`.

## Design (per [[WEIR-I-0044]], Q1 closed)

- **The engine needs NOTHING**: it already commits atomically per `Checkpoint` and handles any number per read (verified, `crates/weir-engine/src/lib.rs:855-959`). This is a guest-local change.
- Per page-batch emission; paginator state (page number / offset / opaque token / last-record cursor) serialized into `StreamState.opaque` (JSON), alongside the datetime cursor in `cursor` as today; `read` resumes from `ctx.state.opaque` when present.
- `MAX_PAGES` becomes a loud `Log`/warning message plus a checkpoint (resumable), never a silent cap.
- Lands together with [[WEIR-T-0186]] (error pages become safe once progress is committed).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] The rest runtime emits one `Records`+`Checkpoint` pair per page-batch for every pagination kind (page, offset, cursor incl. record-field, link-header); memory is bounded by page size, not stream size
- [x] Kill-at-page-N proof: a mock-HTTP engine test interrupts mid-pagination and a second run resumes from the last committed page via `opaque` (no re-read, no gap)
- [x] MAX_PAGES emits a loud warning + committed checkpoint instead of silently truncating
- [x] Existing suites green: wasm_http_engine (incl. Stripe wire test), manifest corpus, importer, `angreal check all` + unit wall

## Status Updates **[REQUIRED]**

- **2026-08-30 — Activated; design settled after verifying the seams.** Confirmed: `fidius_guest::Stream::from_iter` takes any lazy `Iterator + Send + 'static` (fidius-guest stream_marker.rs) — so the guest can yield pages lazily, no fidius change needed. Engine (weir-engine/src/lib.rs:728-751, 855-959) loads `opaque` from stream_state into ReadContext and commits it per Checkpoint — engine untouched, as designed. No codegen template mirrors rest/src/lib.rs (grep fetch_slice: only the one file); serde derive already a guest dep. Implementation plan:
  - `ResumeState` (serde JSON in `StreamState.opaque`): `{v, slice, page, offset, page_cursor, link_next, row_n, header, req_cursor, cursor_seen}`.
  - `ReadIter` (lazy Iterator<ReadMessage>): one HTTP page per step → `Records` + `Checkpoint`; `fetch_slice` refactored into per-page `fetch_page`; substream parents re-fetched (buffered `fetch_all`) on resolve — parent lists are small, drift across attempts is inherent.
  - **Cursor safety**: mid-run checkpoints commit `cursor = base_cursor` (read-start value) so losing opaque only re-reads, never skips; the advancing max rides in opaque as `cursor_seen` and is published + opaque cleared only by the clean-end checkpoint. Side effect (deliberate fix): the incremental cursor request param is now SNAPSHOTTED per slice (`req_cursor`) instead of advancing mid-pagination — the old code shifted the query window between pages of one read (page-skidding).
  - `MAX_PAGES` const → `max_pages` config knob (default 1000), counted **per run**; hitting it = Warn log + committed resumable checkpoint (chunked reads across runs), never silent truncation.
  - Tests (wasm_http_engine.rs): `mock_http_paged` (page-param mock, request log + healthy flag; page 3 answers 500 while unhealthy) → kill-at-page-3 resume test (run1 errs with 2 checkpoints committed; run2 resumes at page 3, wire-asserted `[1,2,3,3,4]`; run3 fresh from page 1 proving opaque cleared) + max_pages=2 test (run1 Ok+warn in run_logs, run2 finishes, no page fetched twice).
- **2026-08-30 — DONE.** Implemented exactly as planned: rest/src/lib.rs `read()` now returns a lazy `ReadIter` (one HTTP page per pull → `Records`+`Checkpoint`); `fetch_slice` → per-page `fetch_page` (all pagination kinds unified); `ResumeState` JSON in opaque with base-cursor safety + `req_cursor` snapshot + `cursor_seen` publish-on-clean-end; substream parents via buffered `fetch_all`; `max_pages` config knob (default 1000, per-run) → Warn + resumable checkpoint; schema advertises it; CHANGELOG Unreleased entry added. **Verified:** new tests `wasm_http_source_resumes_pagination_after_mid_run_failure` (wire-asserted resume, 3 runs) and `wasm_http_source_max_pages_warns_and_resumes` both pass; full wasm_http_engine 27/27; manifest corpus onboards 36/36; `angreal check all` + unit wall green (0 failures). Note for resident mode: the engine sleeps cadence per Checkpoint, so a resident rest source now paces per page — rest runs are scheduled run-once (cadence None), so no practical impact; flagged here for the record.
