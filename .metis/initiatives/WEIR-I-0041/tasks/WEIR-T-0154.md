---
id: post-with-body-requests-in-the
level: task
title: "POST-with-body requests in the declarative rest runtime"
short_code: "WEIR-T-0154"
created_at: 2026-07-15T02:08:20.054070+00:00
updated_at: 2026-07-15T15:17:43.783445+00:00
parent: WEIR-I-0041
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# POST-with-body requests in the declarative rest runtime

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

Let a declarative source manifest declare `http_method: POST` + a templated JSON `request_body` per stream, so
the shared `rest` runtime can drive APIs whose read surface is POST (GA4 `runReport`, Snowflake SQL API
`/api/v2/statements`, Notion's body cursor). This is the known runtime gap recorded in `manifests/README.md`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] A stream can declare method + JSON body; body values support the same templating as `url_base`/params
      (`{{ config[...] }}`) plus pagination/cursor injection **into the body** (`inject_into: body` at a JSON path).
- [ ] `weir-importer` parses the equivalent Airbyte-declarative constructs (`http_method`, `request_body_json`)
      and maps them onto the new manifest fields; fidelity covered in `weir-importer/tests/fidelity.rs`.
- [ ] Datetime-cursor and page/offset/cursor pagination all work when their target is a body field, proven by
      wiremock-style engine tests alongside the existing rest runtime tests.
- [ ] `manifests/README.md` gap list updated; the Notion body-cursor stream imports (parity signal for
      [[WEIR-S-0016]]).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Extend the manifest schema in `weir-manifest` (method + body template on the stream/requester), thread through
`weir-importer` (`MappingSpec`), the flattened guest config (`manifest_stream_to_config` — beware the stringly
config hop flagged in [[WEIR-I-0033]]), and the `rest` guest's request builder. Body injection points mirror the
existing `inject_into` request-parameter machinery.

### Dependencies
None — this is the spine; [[WEIR-T-0158]] and [[WEIR-T-0159]] block on it.

### Risk Considerations
`ConnectorSpec`/guest config are bincode-positional — additive fields must go through the canonical contract
block + codegen regen ([[WEIR-T-0135]] seam) to avoid a breaking wire change.

## Status Updates **[REQUIRED]**

### 2026-07-14 — oriented; design fixed

**Finding:** `http_method`/`request_body`/`request_headers` already exist end-to-end (T-0068). The actual gap
is **param injection into the body**: `fetch_slice` (rest guest) builds pagination/cursor params as query-string
only; the body is static per-connection. The importer's `RequestOption` drops `inject_into` + `field_path`, so
Airbyte `body_json` intent is silently mis-lowered to query params.

**Design (implementing now):**
- `weir-manifest`: new `InjectInto { Query (default) | Body }` on `Incremental` and on
  `Pagination::{Page,Offset,Cursor}`; validation: body injection requires `http_method: POST`. Param names are
  dot-paths when body-injected.
- `weir-importer`: `RequestOption` gains `inject_into` (location) + `field_path`; `body_json`/`body_data` →
  `InjectInto::Body`; dotted `field_path` preserved. Fixes a silent mis-lowering (was treated as query).
- `weir-app` `manifest_stream_to_config`: new flat keys `page_inject_into` / `cursor_inject_into` (only when
  `body`).
- rest guest: `fetch_slice` splits params into query vec + body overlay (dot-path `set_json_path` into the
  rendered `request_body` or `{}`), body rebuilt per page; `send_with_retry` takes the per-page body. Page/
  offset/size inject as JSON numbers; cursors as strings.
- Tests: manifest parse/validate; importer fidelity (body_json paginator + datetime cursor); engine
  `wasm_http_engine.rs` mocks for Notion-style body cursor, GA4-style offset-in-body, datetime-cursor-in-body.
- `manifests/notion.yaml`: add the POST `/v1/search` body-cursor pagination it documents as inexpressible;
  update `manifests/README.md` gap list.

**Wire-safety check:** guest config is a JSON string (not bincode-positional) — additive keys are safe; no
contract regen needed. `ConnectorSpec` untouched.

### 2026-07-15 — implemented + green

All four ACs implemented:
- `weir-manifest`: `InjectInto` enum on `Incremental` + `Pagination::{Page,Offset,Cursor}`; validation
  (body injection ⇒ POST); parse/validate tests. 16/16 pass.
- `weir-importer`: `RequestOption.{inject_into,field_path}` parsed; `option_param` helper; paginator +
  datetime-cursor lowering carries `InjectInto::Body` with dotted paths. Lib test `imports_body_json_injection`
  + fidelity test `airbyte_body_injection_imports_to_the_handwritten_manifest` (struct-equality vs handwritten
  weir manifest). All pass.
- `weir-app`: `manifest_stream_to_config` bakes `page_inject_into`/`cursor_inject_into` flat keys; test
  `manifest_body_injection_maps_to_rest_config`. 5/5 pass.
- rest guest: `fetch_slice` splits query vs body params, per-page body rebuild via dot-path `set_json_path`
  overlay onto rendered `request_body`; `send_with_retry(c, url, body)` (Some ⇒ POST, default
  `content-type: application/json`); injected params imply POST. Numbers inject as JSON numbers.
- Engine proof: 3 new `wasm_http_engine` tests (Notion-style body cursor ×3 pages, GA4-style offset+limit in
  body ×3 pages, nested-dot-path datetime cursor with static-body merge + no-query-leak assertions). 19/19 pass.
  **Debugging note:** guest `wasi:http` POSTs use `transfer-encoding: chunked` — taught `read_full_request` to
  wait for the terminator chunk and added a `request_body()` de-chunker to the test harness.
- `manifests/notion.yaml`: both streams now declare POST + `request_body_json` (filter + page_size) +
  `Notion-Version` header + body-injected `start_cursor` cursor paginator — the constructs its header comments
  called inexpressible. Corpus: 34/34 tier A (`angreal test manifests` green).
- `manifests/README.md` gap sentence updated (POST-with-body + body injection supported; stale Link-header gap
  note corrected — LinkHeader has been supported since T-0070).

Note: legacy `weir-codegen` path intentionally ignores inject_into (runtime-only feature, matches how it
already treats cursor pagination).

### 2026-07-15 — quality gates green; ready for review

- Added a 4th engine test (`wasm_http_source_walks_page_pagination_in_body`) so **all four** param kinds named
  in AC-3 (datetime cursor, page, offset, opaque cursor) are engine-proven with their target in the body.
  `wasm_http_engine`: 20/20.
- `cargo fmt --all` applied; `angreal check all` (fmt + clippy) green.
- Toolchain note (env, not code): the pinned 1.96.1 rustup install was corrupted — `cargo`, `clippy`, and the
  wasm targets were all missing despite the manifest claiming installed; each fixed by component
  remove/re-add.

All acceptance criteria met. Awaiting human review + transition to completed.
