---
id: google-sheets-source-manifest-live
level: task
title: "Google Sheets source manifest + live test"
short_code: "WEIR-T-0160"
created_at: 2026-07-15T02:09:01.919599+00:00
updated_at: 2026-07-16T00:19:26.961563+00:00
parent: WEIR-I-0041
blocked_by: [WEIR-T-0155]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# Google Sheets source manifest + live test

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

A Google Sheets source: `manifests/google-sheets.yaml` reading tabs of a shared spreadsheet via the Sheets API
(`GET /v4/spreadsheets/{id}/values/{range}`), first row = header → records, authored solely from Google's
official API documentation.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] Manifest onboards through the corpus harness; spreadsheet ID + tab names come from connection config;
      auth is `google_service_account` ([[WEIR-T-0155]]) with the readonly Sheets scope.
- [ ] Header-row → field-name mapping produces typed records (values arrive stringly; type inference via the
      existing `StreamSchema::infer` path is acceptable for the demo bar).
- [ ] Full-refresh sync mode (Sheets has no cursor); re-sync replaces cleanly under the existing write modes.
- [ ] Live test against the provisioned spreadsheet ([[WEIR-S-0018]] §1) green in the authed suite.

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
GET-only — no dependency on [[WEIR-T-0154]]. The interesting part is the row-array → record transform
(`values: [[h1,h2],[a,b],...]`): likely a small `map_transforms` capability ("zip first row as keys") rather
than a runtime change. Check what the transform model ([[WEIR-A-0026]]) already offers before adding anything.

### Dependencies
[[WEIR-T-0155]]; live test needs the GCP provisioning from [[WEIR-S-0018]].

### Risk Considerations
Ragged rows (short rows omit trailing cells) — the transform must pad, not error; cover with a fixture test.

## Status Updates **[REQUIRED]**

### 2026-07-16 — design: `header_row` response shape in the guest (not a mapping op)

**Why not the anticipated map_transforms op:** mapping runs per-record with no cross-record state — the header
IS another record, so "zip first row as keys" is inexpressible there. It's a **response-shape** concern, the
family the guest already owns (record_path, single-object, end-of-data): new stream flag `header_row: true` —
the records array is row-arrays whose first row is the header; the guest zips the rest into objects.

Shape decisions:
- Header cells normalize to snake_case field names (trim, lowercase, non-alnum → `_`; empty → `col_<i>`).
- **Ragged rows pad with null** (the flagged risk); extra cells beyond the header are ignored.
- Every record gains **`_row`** (1-based data-row index) — gives the manifest an honest non-null schema field
  (sheet columns are user-defined, unknowable at authoring time) and a natural upsert key so re-syncs replace
  cleanly (AC-3). Real columns pass through schema enforcement untouched; types come from `StreamSchema::infer`
  (AC-2's stated bar).
- One tab per connection v1: path `/v4/spreadsheets/{{ config['spreadsheet_id'] }}/values/{{ config['tab'] }}`
  (config-driven List partitions don't exist; multi-tab = multiple connections, noted in the manifest).

Plumbing: `Stream.header_row` (manifest) → flatten key → guest `RestCfg.header_row` + zip in `fetch_slice`
(header per slice, survives pagination) → importer extension `header_row: true` on DeclarativeStream.
Tests: engine e2e with a sheets-shaped mock (incl. a ragged row + a Filter on a zipped field name to prove the
keys exist), corpus tier A, keyed live via `secrets/google-sheets.json` + example bundle.

### 2026-07-16 — implemented + green; live self-arming

- `header_row` landed across all four layers: manifest `Stream.header_row`, `manifest_stream_to_config` flat
  key, importer `DeclarativeStream.header_row` extension (test `imports_header_row_stream`), rest guest zip in
  `fetch_slice` (`header_name` snake_case normalizer + `_row` index; header held across pages). No new mapping
  op — it's a response-shape concern the guest owns, as designed.
- `manifests/google-sheets.yaml`: GET `/values/{tab}`, `header_row: true`, `google_service_account` +
  spreadsheets.readonly scope, config-templated spreadsheet_id/tab, `_row` primary key. Provenance header.
  **Corpus: 36/36 tier A.**
- Engine e2e `wasm_http_source_zips_header_row_values`: header consumed, data rows zipped to snake_cased named
  fields (proven by a Filter on `email_address`), **the ragged row padded with null (not errored)** — the
  flagged risk. wasm_http suite 24/24; importer 20; fmt+clippy clean.
- Live (AC-4) rides the keyed harness (`secrets/google-sheets.json`, same SA key as GA4); example bundle
  added; self-arms when the bundle lands ([[WEIR-S-0018]] §1). No harness code needed.

**AC status:** 1, 2, 3 verified (corpus + engine e2e w/ ragged row + `_row` upsert key). AC-4 (live) implemented
+ self-arming on `secrets/google-sheets.enc.json` — the human-side step. Awaiting review.
