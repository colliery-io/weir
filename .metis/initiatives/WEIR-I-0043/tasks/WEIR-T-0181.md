---
id: s3-listobjectsv2-continuationtoken
level: task
title: "s3 ListObjectsV2 ContinuationToken loop"
short_code: "WEIR-T-0181"
created_at: 2026-08-29T14:26:41.851751+00:00
updated_at: 2026-08-29T20:07:46.021902+00:00
parent: WEIR-I-0043
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0043
---

# s3 ListObjectsV2 ContinuationToken loop

## Parent Initiative

[[WEIR-I-0043]]

## Objective **[REQUIRED]**

Close the s3 connector's silent truncation: its ListObjectsV2 call has no `ContinuationToken` loop, so buckets with more than 1000 objects sync only the first page and REPORT SUCCESS — the worst failure mode for an ingestion tool (noted at [[WEIR-T-0172]] as out-of-scope there; owned here).

## Approach

- In `crates/connectors/s3`: after each ListObjectsV2 response, follow `IsTruncated`/`NextContinuationToken` until exhausted, feeding keys into the existing read/emit path; the key-cursor incremental semantics must hold across page boundaries (the cursor is the last emitted key, unaffected by listing pagination).
- Memory posture: keys accumulate per listing page, objects stream as today; note any whole-listing buffering honestly (the [[WEIR-I-0044]] streaming-checkpoint work owns deeper fixes).
- MinIO integration coverage: seed >1000 tiny objects (or use a reduced `MaxKeys` in test config to force multi-page listing cheaply — preferred, keeps the seed small).

## Acceptance Criteria **[REQUIRED]**

- [x] A listing spanning multiple pages syncs EVERY object (test forces pagination via small `MaxKeys` — `max_keys: 1` over the 2-object seed)
- [x] Key-cursor incremental resumes correctly across a page boundary (second run picks up after the last key of run one)
- [x] No behavior change for single-page buckets; `angreal check all` + unit wall + the s3 docker-gated suite green
- [x] The manifests/installation claims about s3 stay honest (no doc ever claimed the cap — the note lived only in the review; config_schema now states the listing follows continuation tokens)

## Status Updates **[REQUIRED]**

**2026-08-29 — implemented + verified (ralph run), plus a REAL pre-existing SigV4 bug the gate exposed.**

- `crates/connectors/s3`: the listing now loops `IsTruncated`/`NextContinuationToken` (new dependency-free `xml_tag` helper) until exhausted; new optional `max_keys` config key (`&max-keys=N`, mostly the test lever) in config_schema. Keys accumulate per page, objects stream as before; cursor semantics untouched (last emitted key, page-boundary-agnostic).
- **SigV4 double-encoding bug (pre-existing, `crates/weir-runtime`)**: `canonical_query` percent-encoded the ALREADY-encoded wire query, so any reserved character (`%3D` in continuation tokens; equally `prefix`/`start-after` with escapes) double-encoded → signature mismatch → 403 from MinIO/S3. Fixed decode-then-encode-once with a minimal `uri_decode` (`%XX` only; literal `+` preserved); unit test `canonical_query_encodes_exactly_once` pins it. Every prior s3 test passed only because their query values were escape-free.
- Tests: new docker-gated `s3_wasm_paginated_listing_reads_all_objects` — `max_keys: 1` forces one object per page over the seeded bucket; full refresh reads all 5 records across pages (pre-fix: first page only + false success) and the incremental cursor lands on the LAST page's key and resumes clean. Full s3 suite 3/3 vs MinIO; weir-runtime lib 24/24; `angreal check all` clean; unit wall 12/12.
