---
id: ga4-source-manifest-live-test
level: task
title: "GA4 source manifest + live test"
short_code: "WEIR-T-0159"
created_at: 2026-07-15T02:08:55.984011+00:00
updated_at: 2026-07-16T00:05:36.652808+00:00
parent: WEIR-I-0041
blocked_by: [WEIR-T-0154, WEIR-T-0155]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0041
---

# GA4 source manifest + live test

## Parent Initiative **[CONDITIONAL: Assigned Task]**

[[WEIR-I-0041]]

## Objective **[REQUIRED]**

A Google Analytics (GA4) source: `manifests/google-analytics.yaml` driving the Analytics Data API
(`POST /v1beta/properties/{property}/runReport`) with a representative report stream set (sessions/users by
date + channel, page performance, events), authored solely from Google's official API documentation.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria **[REQUIRED]**

- [ ] Manifest onboards through the corpus harness (`manifest_corpus` tier report) and declares
      `google_service_account` auth + POST bodies; property ID comes from connection config.
- [ ] Streams paginate via the API's `offset/limit` in the request body; date-range incremental sync on the
      `date` dimension checkpoints correctly.
- [ ] Live test against the provisioned GA4 property ([[WEIR-S-0018]] §1) green in the authed suite — gated on
      the 24–48h data-lag window having passed.
- [ ] Provenance header per `manifests/README.md` convention (authored from official docs; Apache-2.0).

## Implementation Notes **[CONDITIONAL: Technical Task]**

### Technical Approach
Pure manifest authoring once [[WEIR-T-0154]] + [[WEIR-T-0155]] exist — this task is deliberately the proving
ground for both. Response shape is columnar (`dimensionHeaders`/`rows[].dimensionValues`) — check the record
selector can flatten it; if not, that's a `map_transforms` addition, not a new runtime feature.

### Dependencies
[[WEIR-T-0154]], [[WEIR-T-0155]]; live test additionally needs the GCP/GA4 provisioning from [[WEIR-S-0018]].

### Risk Considerations
GA4's columnar response is the format risk — validate the selector/transform path against a captured real
response early, before the live window opens.

## Status Updates **[REQUIRED]**

### 2026-07-15 — design fixed: columnar flatten via dot-path mapping + track-only cursor

**Columnar flatten (the flagged risk):** `record_selector: rows` leaves records as
`{dimensionValues:[{value},…], metricValues:[{value},…]}`. The anticipated `map_transforms` addition is two
small, generic upgrades — no new ops, no wire change:
1. `eval_compute`'s field lookup (engine mapping stage) becomes **dot-path aware** (`dimensionValues.0.value`;
   numeric segment = array index) — upgrades Field/Concat/Lower/Upper at once.
2. importer `record_field` parses **chained accessors** (`{{ record['dimensionValues'][0]['value'] }}` →
   `Field("dimensionValues.0.value")`).
Manifest then flattens with AddFields + RemoveFields; metric strings coerce via stream-schema enforcement
(T-0119).

**Cursor (a real GA4 semantics finding):** the guest extracts cursors from **raw** records (pre-mapping), and
GA4's `date` is `YYYYMMDD` while `dateRanges.startDate` only takes ISO/`NdaysAgo` — a naive cursor→param
round-trip is malformed. Deeper: GA4 **restates late-arriving data**, so strict `> cursor` reads undercount;
the correct load pattern is a lookback-window re-read + keyed upsert. Design:
- `Incremental.cursor_param` becomes optional (empty = **track-only**: checkpoint advances + resumes, nothing
  injected into the request).
- New `Incremental.cursor_value_path` — raw-record dot-path for cursor extraction on columnar responses
  (`dimensionValues.0.value`), while `cursor_field: date` stays schema-valid.
- Importer extensions on `DatetimeBasedCursor`: `cursor_value_path`, `track_only`.
- Manifest streams read a config-driven window (`dateRanges: [{startDate: {{ config['start_date'] }}, endDate:
  today}]`) — idempotent under keyed upsert, matches GA4 restatement.

**Live path:** the keyed live harness maps `secrets/<slug>.json` → `manifests/<slug>.yaml` 1:1 — so the GCP SA
key lands in `secrets/google-analytics.json` (and later `google-sheets.json`) with per-connector fields
(`property_id`, `start_date`). Zero harness code; WEIR-S-0018 §1 updated to name the two slugs.

Implementing: mapping+importer path upgrades → manifest fields/flatten/guest `cursor_value_path` → 3-stream
`manifests/google-analytics.yaml` → engine GA4-shape test → corpus + example bundle.

### 2026-07-15 — implemented + green; live case self-arming

- **Path upgrades landed** (generic, no wire change): engine `lookup_path` (dot-path + array-index Field
  resolution; mapping 15/15 incl. the columnar-flatten test) and importer chained-accessor `record_field`
  (importer 19/19 incl. `imports_ga4_columnar_constructs`; junk still rejected → None).
- **Track-only cursor + `cursor_value_path`** through the stack: manifest (`cursor_param` now optional),
  flatten (skips empty param, emits the path), rest guest (`get_path` raw-record extraction), importer
  extensions (`track_only`, `cursor_value_path` on DatetimeBasedCursor).
- **`manifests/google-analytics.yaml`**: 3 streams (traffic, pages, events) — POST `:runReport` with
  config-templated `property_id` path + `start_date` window, body-injected offset/limit (250), AddFields
  dot-path flatten + RemoveFields, track-only date cursor, typed schemas (metric strings coerce at
  enforcement). Provenance header per convention. **Corpus: 35/35 tier A.**
- **Engine e2e** `wasm_http_source_flattens_ga4_columnar_and_checkpoints_track_only_cursor`: two body-offset
  pages of columnar rows through the real guest, flattened by dot-path Computes, static `dateRanges` survives
  the per-page rebuild, **no cursor injected anywhere**, and `store.cursor()` shows `20260703` checkpointed
  from `dimensionValues.0.value`. wasm_http suite 23/23.
- Live: rides the existing keyed harness (`secrets/google-analytics.json` → manifest slug); example bundle
  added; [[WEIR-S-0018]] §1 updated to the two-slug shape. One clippy fix (Boxed RequestOption options —
  large_enum_variant). fmt+clippy clean.

**AC status:** 1, 2, 4 verified (corpus + engine e2e + header). AC-3 (live) self-arms on
`secrets/google-analytics.enc.json` + the 24–48h GA4 data window — the human-side step. Awaiting review.
