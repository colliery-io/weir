---
id: s6-in-flight-transforms-request
level: task
title: "S6: In-flight transforms + request options (POST body, headers, params)"
short_code: "WEIR-T-0068"
created_at: 2026-07-03T01:27:59.491962+00:00
updated_at: 2026-07-04T02:31:31.113449+00:00
parent: WEIR-I-0008
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0008
---

# S6: In-flight transforms + request options (POST body, headers, params)

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0008]] — slice S6. Tracked in [[WEIR-S-0016]] (record-select + request-options rows).

## Objective **[REQUIRED]**

Close the transform + request-shaping declarative gaps. Two layers:
- **Transforms (engine):** Airbyte `AddFields` / `RemoveFields` / key transforms + `record_filter` map
  onto the **existing in-flight mapping stage** ([[WEIR-T-0052]] — `MappingOp` Compute/Drop/Rename/Filter),
  so the importer emits a `MappingSpec` on the `ConfiguredStream` and the engine applies it (no runtime
  change).
- **Request options (rest runtime):** `request_parameters`, `request_headers`, and
  `request_body_json`/`request_body_data` with the **POST** method — the runtime currently only GETs, so
  POST-with-body connectors (e.g. **notion**) can't run yet.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] **Importer → mapping:** `AddFields` → `MappingOp::Compute`, `RemoveFields` → `Drop`, key rename →
  `Rename`, `record_filter` → `Filter`, emitted as the stream's `MappingSpec`. Unsupported transform
  sub-forms reported (not dropped). Unit tests.
- [ ] **Runtime request options:** `rest` supports static `request_parameters` + `request_headers`, and
  **POST** with `request_body_json` / `request_body_data`; importer maps them from the Airbyte requester.
- [ ] **Wire-level proof:** a POST-with-body request over real `wasi:http` (mock asserts method+body); a
  transform test (AddFields/RemoveFields/record_filter change the emitted records via the engine mapping).
- [ ] **notion** (POST body) imports + runs end-to-end (once its key lands via [[WEIR-T-0067]]); until then,
  the wire test proves the POST path.
- [ ] **Ledger flipped:** [[WEIR-S-0016]] `record_filter`, `AddFields/RemoveFields`, `request_body_json`,
  `request_headers`, POST-method rows → ✅. `analyze()` stops reporting them.
- [ ] Workspace + integration suites green; clippy clean.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
- Mapping stage already exists (`weir-engine`, [[WEIR-T-0052]]); this is mostly **importer** work +
  wiring the `MappingSpec` onto the configured stream. Confirm how `MappingSpec` reaches the engine at run
  time (via `ConfiguredStream`), and that the app/orchestrator carries it.
- rest runtime: generalize `fetch_slice` to issue POST (method + body) when configured; add static
  headers/params to the request build. Auth headers still injected host-side ([[WEIR-A-0033]]).

### Dependencies
- Builds on [[WEIR-T-0052]] (mapping) + the rest runtime. Independent of [[WEIR-T-0069]]/[[WEIR-T-0070]].

### Risk Considerations
- POST body may itself be templated (`{{ config[...] }}` / cursor) — reuse the runtime's template pass.
- Keep transforms within the dbt boundary ([[WEIR-A-0026]]) — no joins/aggregates.

## Status Updates **[REQUIRED]**

### 2026-07-04 — request options landed; transforms split to [[WEIR-T-0071]]

**Scope decision:** the two halves have very different shape/effort, so I split them.
- **Request options (POST body + static headers) — ✅ done here.** Self-contained, unlocks POST connectors
  (Notion needs POST + a JSON body + the `Notion-Version` header).
- **Transforms — split to [[WEIR-T-0071]].** They need a `work_spec(&Connection)` → source-manifest →
  `ConfiguredStream.mapping` plumb (no manifest access there today) — architecturally distinct + lower
  value (no corpus connector requires transforms for basic function).

**Implemented (request options):**
- `rest` runtime: `send_with_retry(&RestCfg, url)` builds the request per `http_method` (GET/POST),
  `request_body` (config-templated), and static `request_headers`; auth still host-injected. `fetch_slice`
  routes through it.
- `weir-manifest`: `Stream` gains `http_method` / `request_body` / `request_headers`.
- `weir-importer`: maps the Airbyte requester's `http_method` + `request_body_json` (→ JSON body string,
  added `serde_json` dep) + `request_headers`. Unit test.
- `weir-app`: emits them.
- **Wire test green:** the connector issues `POST /query` with body `{"page_size":100}` and header
  `Notion-Version: 2022-06-28`; record flows through.

**Ledger:** [[WEIR-S-0016]] POST-method / `request_body_json` / static `request_headers` rows → ✅.
Body-cursor pagination (cursor in the POST body — full Notion paging) split into a reported ❌ row.
Transforms rows point to [[WEIR-T-0071]].

**Verification:** importer unit tests + POST wire test (see the batch run). Task **complete** for the
request-options scope; transforms tracked separately.
