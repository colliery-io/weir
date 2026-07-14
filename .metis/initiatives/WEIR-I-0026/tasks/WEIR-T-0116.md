---
id: rest-dest-applies-deletes
level: task
title: "rest-dest applies deletes"
short_code: "WEIR-T-0116"
created_at: 2026-07-07T11:34:15.405148+00:00
updated_at: 2026-07-07T14:40:44.017248+00:00
parent: WEIR-I-0026
blocked_by: [WEIR-T-0113]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0026
---

# rest-dest applies deletes

## Parent Initiative

[[WEIR-I-0026]] — deletes land over HTTP too.

## Objective

The rest destination honors a change's op: on **Delete**, issue the configured delete call (an HTTP `DELETE`
to a key-templated URL); Insert/Update write as today. Signing/auth stays host-side ([[WEIR-A-0033]]).

## Reference

- `crates/connectors/rest-dest/src/lib.rs` — the HTTP destination write path.
- `crates/connectors/rest/src/lib.rs` — `fidius_guest::http` API + template substitution (`{{ }}`) for URLs.
- Config: a `delete_path`/`delete_method` (default `DELETE`) + how the key maps into the URL (e.g.
  `.../items/{{ record['id'] }}`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `Changes`: Insert/Update → `write_one`; **Delete** → `delete_one` (HTTP `DELETE`/`delete_method` to the
  key-templated `delete_path` URL; 404 also settles).
- [x] Config `delete_path` template + `delete_method` (default `DELETE`); missing `delete_path` → the delete
  dead-letters with a clear reason.
- [x] Plain `Rows` unchanged; `send_with_retry` gained a `method` param, same retry/backoff.
- [x] Unit test on the URL templater (`render_path` moved to connector-types); rest-dest wasm + workspace + clippy clean.

## Status Updates

### 2026-07-07 — done (`983c204`)

The write loop is op-aware: Delete → `delete_one` renders `delete_path` with the record key + issues an HTTP
`DELETE` (configurable method; 404 = already gone); Insert/Update → the existing `write_one`. Missing
`delete_path` → dead-letter (visible misconfig). `send_with_retry(c, url, method, body)` parameterized. The
`{{ record.<field> }}` templater `render_path` moved to `weir-connector-types` (host-testable) with a unit test;
both upsert + delete URLs use it. rest-dest wasm + workspace build; clippy clean. **Complete.**
