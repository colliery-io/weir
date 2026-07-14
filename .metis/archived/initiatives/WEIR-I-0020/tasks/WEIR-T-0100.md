---
id: openlineage-runevent-emission
level: task
title: "OpenLineage RunEvent emission"
short_code: "WEIR-T-0100"
created_at: 2026-07-06T01:52:37.520530+00:00
updated_at: 2026-07-06T02:05:46.193989+00:00
parent: WEIR-I-0020
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0020
---

# OpenLineage RunEvent emission

## Parent Initiative

[[WEIR-I-0020]] — the distinctive open-standard piece. Governed by [[WEIR-A-0022]] decision 1 (OpenLineage).

## Objective

Emit **OpenLineage `RunEvent`s** for every sync run — asynchronously (never blocking the run path) — so weir's
lineage is interoperable with Marquez / DataHub / any OpenLineage consumer.

## Reference

- OpenLineage RunEvent: `{eventType: START|COMPLETE|FAIL|ABORT, eventTime, run:{runId}, job:{namespace,name},
  inputs:[dataset], outputs:[dataset], producer}`; datasets `{namespace, name}` + facets.
- The run lifecycle: `plan_run`/`Relay::plan` (a run begins) → engine `execute` → `mark_done`/dead-letter
  (`crates/weir-engine`, `weir-orchestrator`). `work_units` carries source_ref/dest_ref/stream/rows_written/
  tenant/timing — the raw material for the events.
- Emit as raw JSON (a small typed model) — no heavy SDK needed; POST to the transport.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] A `weir-lineage` surface (crate or module) emits OpenLineage `RunEvent`s: **START** when a run leases,
  **COMPLETE**/**FAIL** when it finishes; `job` = `weir/<tenant>/<connection>`, `inputs` = source dataset
  (source connector + stream), `outputs` = dest dataset; facets: `outputStatistics` (rowCount = rows_written) +
  run timing; `producer` = weir version.
- [ ] **Pluggable transport** via config: `WEIR_OPENLINEAGE_URL` (HTTP POST to an OL endpoint), `console`
  (stderr/log), or **off** (default). Emission is async + best-effort — a transport failure never fails the run.
- [ ] Wired into the run lifecycle (engine/orchestrator) without blocking; events fire for a real run.
- [ ] A test asserts the START + COMPLETE event shapes (valid OpenLineage JSON) for a run; clippy clean.

## Status Updates

### 2026-07-06 — done (`df7a5c1`)

`weir-orchestrator/src/lineage.rs`: `Lineage` (transport from `WEIR_OPENLINEAGE_URL`: http POST / `console` /
off-default) + `run_event()` building a spec-shaped OpenLineage `RunEvent`. Wired into `Worker::tick`: **START**
on lease (best-effort spec load), **COMPLETE/FAIL** on the result — `tokio::spawn` fire-and-forget for http, so
it **never blocks or fails the run**. `job`=`weir/<tenant>/<connection>`, inputs/outputs=source/dest datasets,
`outputStatistics.rowCount` facet, a deterministic UUID-shaped runId (from the work-unit id, no uuid dep).

**Verified end-to-end**: `WEIR_OPENLINEAGE_URL=console weir run --name demo` emitted valid START + COMPLETE
(shared runId, rowCount=1). Unit test asserts the event shapes. clippy clean. Deps: `reqwest` (rustls). Note:
the orchestrator's `partitioned_plan` test is flaky under **parallel** sqlite (`database is locked`) — passes
serially (the angreal lane runs `--test-threads=1`); not lineage-related. **Complete.**
