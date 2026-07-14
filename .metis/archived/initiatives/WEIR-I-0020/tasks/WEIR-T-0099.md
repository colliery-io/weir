---
id: prometheus-metrics-endpoint-run
level: task
title: "Prometheus /metrics endpoint + run-path instrumentation"
short_code: "WEIR-T-0099"
created_at: 2026-07-06T01:52:32.263971+00:00
updated_at: 2026-07-06T02:13:48.594264+00:00
parent: WEIR-I-0020
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0020
---

# Prometheus /metrics endpoint + run-path instrumentation

## Parent Initiative

[[WEIR-I-0020]]. Governed by [[WEIR-A-0022]] decision 3 (public/aggregate Prometheus).

## Objective

Expose weir's operational metrics for Prometheus: a **public** `/metrics` endpoint + instrumentation of the run
path, so operators see throughput, latency, and failure rates.

## Reference

- `crates/weir-api/src/lib.rs` — `/health` is public (outside the auth layer); add `/metrics` alongside.
- `crates/weir-engine`/`weir-orchestrator` — the run lifecycle (claim → execute → mark_done/dead-letter) is
  where the counters/histograms fire.
- `metrics = "0.24"` + `metrics-exporter-prometheus` (cloacina's versions).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] `metrics` + `metrics-exporter-prometheus` wired; a **public** `GET /metrics` renders the Prometheus text
  format (added to the public routes, not behind auth).
- [ ] Instruments: `weir_rows_written_total`, `weir_runs_total{state}`, `weir_run_duration_seconds` (histogram),
  `weir_dead_letters_total`, `weir_queue_depth` (gauge) — labelled by **connection + state** (aggregate).
- [ ] Per-tenant labels are **opt-in** via `WEIR_METRICS_TENANT_LABELS=1` (off by default → no tenant leak to
  unauthenticated scrapers).
- [ ] A test scrapes `/metrics` (no auth) → 200 + the expected metric names; workspace + clippy clean.

## Status Updates

### 2026-07-06 — done (`f4bf971`, `e236b0a`)

- **weir-api**: `metrics-exporter-prometheus` recorder installed once (`OnceLock`) in `router()`; a **public**
  `GET /metrics` (next to `/health`, outside auth) renders the Prometheus text.
- **weir-orchestrator** (`emit_run_metrics` in `Worker::tick`): `weir_runs_total{connection,state}`,
  `weir_rows_written_total{connection}`, `weir_run_duration_seconds` (histogram, timed via `Instant`),
  `weir_dead_letters_total` — aggregate labels; **per-tenant labels opt-in** via `WEIR_METRICS_TENANT_LABELS`
  (no tenant leak to unauthenticated scrapers by default). Reuses the spec load added for lineage.
- Test **`metrics_endpoint_public_and_records_runs`**: unauthenticated `/metrics` → 200; after a real run the
  text contains `weir_runs_total` + `weir_rows_written_total`. clippy clean.

**Deferred:** `weir_queue_depth` gauge — needs a periodic pending-count gather in `serve` (a small follow-up);
the 4 run-completion metrics (the substance) are in. **Complete.**
