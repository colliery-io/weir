---
id: url-encode-outbound-query-params
level: task
title: "URL-encode outbound query params (rest runtime + append_query)"
short_code: "WEIR-T-0185"
created_at: 2026-08-30T11:53:58.730281+00:00
updated_at: 2026-08-30T12:23:42.885407+00:00
parent: WEIR-I-0044
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0044
---

# URL-encode outbound query params (rest runtime + append_query)

## Parent Initiative

[[WEIR-I-0044]]

## Objective **[REQUIRED]**

Silent-corruption class: outbound query parameters are never URL-encoded — an ISO timestamp cursor with `+` (e.g. `2026-01-01T00:00:00+02:00`) arrives at the API as a space, and any `&`/`=`/`#` in a cursor or config value corrupts the query string. Encode values exactly once at the two build sites: the rest runtime's query assembly (`crates/connectors/rest` fetch_slice) and the host-side `append_query` (`crates/weir-runtime`, the api-key-as-query credential path).

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [x] Query values with `+ & = # %` and spaces reach the wire percent-encoded exactly once (no double-encoding of already-encoded opaque cursor tokens — decide and document per site which form each receives)
- [x] `append_query` encodes the credential value; the SigV4 canonicalization (fixed in [[WEIR-T-0181]]) still agrees with the wire form
- [x] Wire-level test: a mock server asserts the RAW query it received for a `+`-bearing datetime cursor and a reserved-char config param
- [x] `angreal check all` + unit wall + wasm_http_engine green

## Status Updates **[REQUIRED]**

- **2026-08-30 — DONE.** Two build sites fixed, values-only, RFC 3986 unreserved-set encoding:
  - **Guest** (rest/src/lib.rs): new `url_encode` applied in `fetch_page` to the three STRING-valued query params — opaque page token (`page_cursor`), incremental cursor (`req_cursor`), `cursor_end`. Documented form decisions in the helper's doc comment: values are assumed RAW (response tokens/record fields/config datetimes — base64 cursors need `+/=` encoded; APIs that hand back pre-encoded continuations do so as full Link-header URLs, which are spliced verbatim); param NAMES go verbatim (author-controlled, `filter[updated]` stays literal); numeric page/offset/size params are no-ops; body-injected params are JSON values, untouched.
  - **Host** (weir-runtime `append_query`): credential value percent-encoded once via the existing `uri_encode(_, false)`; SigV4 agreement holds by construction — `canonical_query` (T-0181) decodes the wire query then re-encodes once, so `%2B` → `+` → `%2B`.
  - **Tests:** new wire test `wasm_http_source_percent_encodes_query_values` (mock captures the RAW request line: `since=2020-01-01T00%3A00%3A00%2B02%3A00`, host-injected `apikey=k%2B%26%3D%23z`, plus a no-`%25`-anywhere double-encoding guard); runtime unit `append_query_percent_encodes_the_credential_value`; existing datetime-bounds test updated to the encoded wire form. wasm_http_engine 30/30, manifest corpus 36/36, `angreal check all` + unit wall clean. CHANGELOG Fixed entry added.
