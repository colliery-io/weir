---
id: flagship-manifest-fidelity
level: task
title: "Flagship manifest fidelity — pagination (+incremental) for stripe/hubspot"
short_code: "WEIR-T-0168"
created_at: 2026-08-16T15:24:06.182973+00:00
updated_at: 2026-08-25T03:04:07.951973+00:00
parent: WEIR-I-0042
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0042
---

# Flagship manifest fidelity — pagination (+incremental) for stripe/hubspot

## Parent Initiative

[[WEIR-I-0042]]

## Objective **[REQUIRED]**

The two flagship SaaS manifests silently truncate: stripe.yaml and hubspot.yaml omit pagination (with in-file "⚠ unconfirmed" notes) although the shared declarative runtime supports opaque-cursor paging — a Stripe sync returns ~10 records and reports success, which is worse than failing. Add pagination (+ incremental where the API supports it) so the flagship manifests are correct, and sweep the other unpaginated manifests.

## Evidence (2026-08-16 alpha review)

- `manifests/stripe.yaml:40-46`, `manifests/hubspot.yaml:26-28` — no paginator, "⚠ unconfirmed" field notes; Stripe also lacks incremental.
- Runtime support already exists: opaque-cursor + POST-body param injection in `crates/connectors/rest/src/lib.rs`.
- Also unpaginated per the connectors survey: airtable, todoist, openweather.
- Constraint to verify: opaque-cursor tokens must be JSON strings (`.as_str()`, `crates/connectors/rest/src/lib.rs:698-703`) and there is NO has_more-boolean stop-condition support — HubSpot's `paging.next.after` is a string (fine); Stripe pages by `starting_after` = last object id + `has_more`, which may need a small runtime/importer addition.

## Acceptance Criteria **[REQUIRED]**

- [x] stripe.yaml pages through full result sets — the runtime gap was real and is IMPLEMENTED: `page_cursor_record_field` (token = last record's field, Stripe `starting_after`) + `page_stop_on_false_path` (`has_more` stop, no wasted empty-page request) + `limit=100`. Incremental stays documented-unexpressible (Stripe's `created[gte]` = nested param + epoch ints vs DatetimeBasedCursor's ISO + flat param) — noted in-manifest, unchanged claim
- [x] hubspot.yaml pages via the opaque `paging.next.after` cursor (`?after=…&limit=100`) on all three streams
- [x] airtable / todoist / openweather checked: airtable's "not expressible" comment was stale — its response `offset` token is a standard opaque cursor, now configured (`pageSize=100`); todoist (REST v2 full arrays) and openweather (single-object responses) correctly have no paginator
- [x] `angreal test manifests` green (36/36 tier A incl. the three changed); new engine test `wasm_http_source_walks_stripe_last_record_cursor` proves the Stripe shape wire-level (the mock exits after the `has_more:false` page, so an extra request would fail); the full http suite is 25/25
- [x] Live verification recorded as pending [[WEIR-T-0067]] secret bundles — nothing here is claimed live-verified

## Implementation Notes

If a runtime addition is needed for Stripe (cursor-from-last-record-id and/or has_more stop condition), add it to the shared runtime in `crates/connectors/rest` with a mock-HTTP engine test, mirror it in the importer's construct mapping, and note it in the declarative parity ledger (WEIR-S-0016). Watch the interaction with the whole-stream-in-memory read: full pagination on a large Stripe account will buffer everything — acceptable for alpha, but note it; MAX_PAGES=1000 still caps.

## Status Updates **[REQUIRED]**

**2026-08-25 — implemented across the full seam (ralph run).**

- **Runtime** (`crates/connectors/rest`): two new config keys — `page_cursor_record_field` (next-page token from the LAST record's dot-path field) and `page_stop_on_false_path` (response bool dpath; false = last page). Last-record token captured before the rows loop consumes the page; stop check runs before the advance; config_schema updated.
- **Model** (`weir-manifest`): `Pagination::Cursor` gains `cursor_record_field`, `stop_on_false_path`, `size_param`, `size` (all `#[serde(default)]` — stored manifests round-trip unchanged).
- **Importer** (`weir-importer`): `CursorPagination` accepts `stop_condition` + `page_size`; `{{ last_record['id'] }}` lowers to the record-field cursor and the `not response['has_more']` idiom lowers to the stop path (new `bracket_path` helper generalizing `response_path` with contiguous-bracket parsing). New lowering test `imports_stripe_style_last_record_cursor`; suite 21/21.
- **Baking** (`weir-app`): the Cursor arm emits the new keys + the page-size pair (the runtime appends `limit` for any strategy).
- **Manifests**: stripe (both streams: `starting_after`/`has_more`/limit 100), hubspot (all three streams: `after`/limit 100), airtable (`offset` token/pageSize 100 — its "not expressible" comment was stale). todoist/openweather verified correctly unpaginated.
- **Ledger** ([[WEIR-S-0016]]): new last-record+stop row ✅ (stripe); body-cursor row flipped ❌→✅ (was stale since [[WEIR-T-0154]] — the engine body-cursor test proves it); changelog entry added.
- Verified: importer 21/21, http engine 25/25 (incl. the new Stripe wire test), manifest corpus 36/36 tier A, `angreal test unit` all green, `angreal check all` clean.
- Known bound (pre-existing, [[WEIR-I-0044]]'s scope): full pagination buffers the whole stream in memory before one checkpoint; MAX_PAGES=1000 still caps.

**2026-08-28 — post-review fix (pre-v0.0.1 release review).** The adversarial release review confirmed a polarity bug in the importer lowering: `map_paginator` lowered ANY response-referencing `stop_condition` to `stop_on_false_path` — but Airbyte's contract is "stop when the template is TRUE" while the runtime key means "stop when the field is FALSE", so a POSITIVE condition (`{{ response['is_last_page'] }}`, Spring-style paging) would invert and silently truncate every multi-page sync after page 1. Fixed with a polarity guard: only the negated idiom (`not …` / `… is false`) lowers; positive conditions are dropped and cursor-absence terminates. New test `positive_stop_condition_is_dropped_not_inverted` covers all three shapes; importer 22/22. Also aligned the `cursor_record_field` doc in weir-manifest with the runtime's actual precedence (a set `cursor_path` wins, record-field is the fallback).
