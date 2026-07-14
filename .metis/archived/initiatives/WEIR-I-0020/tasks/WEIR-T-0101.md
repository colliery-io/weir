---
id: ui-run-detail-lineage-panel
level: task
title: "UI run-detail lineage panel"
short_code: "WEIR-T-0101"
created_at: 2026-07-06T01:52:42.051982+00:00
updated_at: 2026-07-06T02:23:15.806394+00:00
parent: WEIR-I-0020
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0020
---

# UI run-detail lineage panel

## Parent Initiative

[[WEIR-I-0020]] — the operator-facing view. Complements the OpenLineage emission ([[WEIR-T-0100]]).

## Objective

Show a run's lineage in the UI: for a run, the **source → stream → mapping → dest** chain + rows/timing —
derived from the existing run data (no new store).

## Reference

- `weir-ui/src/main.rs` — the Operations run-detail modal ([[WEIR-I-0016]]) is where the lineage renders.
- Data: `work_units` (source_ref/dest_ref/stream/rows_written/timing) + the connection's mapping; a small API
  (`GET /connections/{name}/runs` already returns run rows; add a lineage shape if needed) or reuse existing.
- Aurora components (`Group`/`Pill`/`Panel`; a graph/list) for the chain.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] The run-detail view shows the lineage chain: **source connector · stream → mapping → dest connector**, with
  rows written + duration.
- [ ] Reads from existing run data (no schema change); tenant-scoped like the rest of the UI ([[WEIR-I-0019]]).
- [ ] wasm builds; `angreal ui build`; an e2e assertion (or extends `tenant`/`operations` spec) that the lineage
  chain renders for a run; clippy clean.

## Status Updates

### 2026-07-06 — done (`1387212`) — closes I-0020

The run-detail modal (`weir-ui/src/main.rs`) gains a **Lineage** section: `source · stream → dest` + rows +
duration, derived from the selected connection + its latest run (`connections`/`runs` signals) — **no schema
change**, tenant-scoped like the rest of the UI ([[WEIR-I-0019]]). (The mapping is implicit — the manifest→config
bake isn't surfaced separately; the source→dest chain + metrics is the operator view; full lineage is the
OpenLineage stream, [[WEIR-T-0100]].)

`operations.spec.ts` asserts the **Lineage** section renders on the modal; wasm builds; **full e2e 7/7 green**;
clippy clean. **Complete — closes [[WEIR-I-0020]].**
