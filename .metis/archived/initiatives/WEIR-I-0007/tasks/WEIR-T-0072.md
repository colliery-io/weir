---
id: s2-shared-declarative-destination
level: task
title: "S2: Shared declarative destination runtime + destination manifest schema"
short_code: "WEIR-T-0072"
created_at: 2026-07-04T03:17:19.545508+00:00
updated_at: 2026-07-04T03:38:28.234310+00:00
parent: WEIR-I-0007
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0007
---

# S2: Shared declarative destination runtime + destination manifest schema

## Parent Initiative

[[WEIR-I-0007]] (first-class reverse ETL), slice S2. Architecture: [[WEIR-A-0034]] — destinations are a
**shared declarative runtime**, mirroring the source model of [[WEIR-A-0032]] (NO `weir-codegen`).

## Objective

Build a **new WASM `wasi:http` destination guest** that interprets a **destination manifest** as config at
run time and performs the streaming `write` to a SaaS REST API — the reverse-ETL analogue of
`crates/connectors/rest` (which is the shared *source* runtime). This is the load-bearing slice: once it
exists, HubSpot/Salesforce (S4/S5) are largely manifests + tests, not new engine code.

## Prior art to mirror (read these first)

- `crates/connectors/rest/src/lib.rs` — the shared **source** runtime: `parse_cfg` (manifest-as-config),
  `render_template`, host-auth handled host-side ([[WEIR-A-0033]]), `send_with_retry` (429/5xx backoff,
  [[WEIR-T-0069]]), `Request::post` with body + static headers ([[WEIR-T-0068]]). **Reuse these patterns.**
- `crates/connectors/postgres/src/lib.rs` — the existing **write** guest: the `write(ctx, Stream<RecordBatch>)
  -> WriteOutcome` contract, `RecordBatch::Rows(Vec<String>)` (JSON rows), per-record `DeadLetter{record,
  reason}`, `WriteMode` (Append/Overwrite/Upsert{business_keys}), batch bisection for isolating bad rows.
- Crate layout to copy: `crates/connectors/{rest,postgres}/` (build.rs, Cargo.toml, wit/, src/) built for
  `wasm32-wasip2`; staged into a fidius package with the `http` capability (see the `wasm_http_engine` test
  harness `build_and_stage`).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Destination manifest schema** (`weir-manifest`): a `Destination`/`DestStream` shape declaring, per
  object — `path` (endpoint, templated), `method` (`POST`/`PATCH`), **upsert/business key(s)**, a **field
  map** (record field → SaaS property; identity default), optional `batch_size`, and the auth scheme (reuse
  the existing `Auth` enum). Round-trips YAML. Unit test.
- [ ] **New destination guest crate** (`crates/connectors/rest-dest` or similar) built for `wasm32-wasip2`,
  implementing the connector contract with `roles = [Destination]` and a real `write`:
  - reads its manifest/config via a `parse_cfg` mirroring the source runtime;
  - consumes the `Stream<RecordBatch>` (JSON rows), **shapes each record** into the SaaS object JSON via the
    field map, and issues `POST`/`PATCH` per record (or per batch where the API supports it) over
    `fidius_guest::http`;
  - **upsert semantics**: create-or-update keyed on the business key (the manifest says how — e.g. PATCH to
    `/objects/{key}` or a vendor upsert endpoint);
  - returns `WriteOutcome{ accepted, dead_letters }` — a per-record 4xx becomes a `DeadLetter{record, reason}`,
    not a failed sync.
- [ ] **Reuse, don't reinvent:** host-side credential injection ([[WEIR-A-0033]]) — the guest sends plain
  requests, the host injects auth; transient retry/backoff ([[WEIR-T-0069]] `send_with_retry` pattern).
- [ ] **Wire test** (mirror `crates/weir-engine/tests/wasm_http_engine.rs`): a mock SaaS server; the guest
  upserts N records with the correct method + shaped body + key; one record the mock 4xx-rejects lands in
  `dead_letters`; `accepted` count is correct. Real `wasi:http` over the engine.
- [ ] Workspace + integration suites green; clippy clean; no attribution trailers on commits.

## Technical Notes

- **Scope this slice to the runtime + a generic/mock destination** — do NOT build HubSpot/Salesforce here
  (those are S4/S5). Prove the mechanism against a mock SaaS server.
- The write is **client-streaming** ([[WEIR-A-0029]]); pull `RecordBatch`es until the stream ends, then
  return the outcome. Batch to the SaaS API where it supports bulk create/update; otherwise per-record.
- **Idempotency** is the upsert's job (create-or-update by business key), so replay re-upserts cleanly
  ([[WEIR-A-0011]]) — the full replay proof is S3 ([[WEIR-T-0073]]).
- Keep the field map within the dbt boundary — pure record→object shaping (the heavy transforms already ran
  in the mapping stage, [[WEIR-T-0071]]).

## Dependencies

- Prereq for [[WEIR-T-0073]] (flow), [[WEIR-T-0074]] (HubSpot), [[WEIR-T-0075]] (Salesforce).
- S1 (mapping) already shipped ([[WEIR-T-0071]]); host-auth + retry from the [[WEIR-I-0008]] arc are reused.

## Status Updates

### 2026-07-04 — implemented + wire-test green

**New crate `crates/connectors/rest-dest`** (standalone `wasm32-wasip2` workspace, `capabilities=["http"]`,
mirrors the `rest`/`postgres` scaffolding). `RestDest` implements the connector contract with
`roles=[Destination]` and a real client-streaming `write`:
- `parse_cfg` → `DestCfg { base_url, path, method, field_map, body_wrap, max_retries }` (manifest-as-config).
- `write` pulls `RecordBatch::Rows`, and per record: `shape_body` (field map → SaaS properties, optional
  `body_wrap` like HubSpot's `properties`), `render_path` (`{{ record.<field> }}` for upsert-by-key URLs),
  then `send_with_retry` (POST/PATCH via `Request::post` + `req.method` override; `content-type: json`).
- 2xx → `accepted`; 4xx → `DeadLetter{record, reason}` (no failed sync); 429/5xx → backoff retry
  (`backoff_ms`/`retry_after_ms`, reused from [[WEIR-T-0069]]). Auth host-injected ([[WEIR-A-0033]]).

**Destination manifest schema** in `weir-manifest`: `DestinationManifest { spec, auth, base_url, objects }`
+ `DestObject { name, path, method, upsert_key, field_map, body_wrap, batch_size }` + `from_yaml`
(validates non-empty name/objects). Round-trips YAML.

**Tests green:**
- `weir-manifest`: `parses_destination_manifest`, `destination_manifest_rejects_empty_objects` (2/2).
- `crates/weir-engine/tests/wasm_rest_dest_engine.rs`: rest source → rest-dest → mock SaaS over real
  `wasi:http` — 3 records, one 4xx-rejected → **2 accepted + 1 dead-letter** (store), correct `POST /contacts`
  + shaped body (`name`→`fullname`). `dead_letter_count` is on `Store` (not `Engine`); `rows_written` sums
  `receipt.accepted`.

**Scope note:** the guest keys the method off `config.method` (POST/PATCH); mapping `WriteMode::Upsert`
→ method/keyed-path is [[WEIR-T-0073]]. Broader catalog/docker registration deferred to when a real dest
(S4/S5) onboards. Bulk batching = per-record for now (noted for S5).

**Remaining:** clippy clean (running) → commit → complete.
