---
id: observability-tracing-metrics
level: initiative
title: "Observability — tracing, metrics, lineage"
short_code: "WEIR-I-0020"
created_at: 2026-07-05T23:46:06.012430+00:00
updated_at: 2026-07-06T02:23:21.146546+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: observability-tracing-metrics
---

# Observability — tracing, metrics, lineage Initiative

## Context

weir has run logs + dead-letters + the new audit trail ([[WEIR-I-0017]]), but no **operational** observability:
no metrics, no distributed tracing/OTel export, no run **lineage** view. To operate weir (and especially the
tenancy + k8s work — [[WEIR-I-0018]]/[[WEIR-I-0021]]), you need to see throughput, latency, failure rates, and
trace a sync end-to-end. This brings weir to cloacina's observability bar: structured tracing + OpenTelemetry
(OTLP) + Prometheus metrics + lineage — spec [[WEIR-S-0011]] (Observability & Lineage), ADR [[WEIR-A-0022]]
(draft, to be decided here).

## Goals & Non-Goals

**Goals**
- **Structured tracing** — `tracing` with JSON output + an env-driven filter; spans across the request →
  orchestrator → engine → connector path, carrying `connection`/`run`/`tenant` ids. Mirror cloacina's
  `tracing-subscriber` JSON setup.
- **OpenTelemetry export** — an optional `telemetry` feature exporting traces + metrics via **OTLP** (behind a
  cargo feature so the default build stays light). Mirror cloacina's `tracing-opentelemetry` + `opentelemetry-otlp`.
- **Prometheus metrics** — a `/metrics` endpoint (`metrics` + `metrics-exporter-prometheus`): rows written,
  run count by state, run duration histogram, dead-letters, queue depth, connector latency — labelled by
  connection + tenant. Mirror cloacina `ops_metrics.rs`.
- **Run lineage** — a lineage view/API: for a run, the source→stream→mapping→dest chain + rows/bytes + timing;
  surface it in the UI ([[WEIR-I-0016]] run-detail) — Aurora ships a Graph/DAG component to draw it.
- **Decide [[WEIR-A-0022]]** (Observability & lineage standard) as part of this.

**Non-Goals**
- A bundled Grafana/Prometheus stack (we *expose* metrics/traces; dashboards are the operator's / periphery).
- Log aggregation / long-term retention (periphery).
- Alerting rules.

## Prior art (cloacina)

`~/Desktop/cloacina/crates/cloacina-server/src/ops_metrics.rs`; the `telemetry` cargo feature
(`tracing-opentelemetry`, `opentelemetry`, `opentelemetry-otlp`, `opentelemetry_sdk`); `metrics` +
`metrics-exporter-prometheus`; `tracing-subscriber` (env-filter + json).

## Weir surfaces to change

- `crates/weir-cli` — the `tracing_subscriber` init already exists; add JSON output + the optional OTel layer.
- `crates/weir-api` — a public (or admin) `/metrics` endpoint; instrument handlers.
- `crates/weir-engine` / `weir-orchestrator` — spans + metrics on the execution path (per run/connection/tenant).
- `crates/weir-app` — a lineage query for a run (from `work_units` + `run_logs` + the mapping).
- `weir-ui` — a lineage panel in run-detail (Aurora `Graph`/DAG).
- ADR: finalize [[WEIR-A-0022]]; spec [[WEIR-S-0011]].

## Design decisions (2026-07-06, approved)

1. **`/metrics` = public, aggregate by default, tenant labels opt-in.** Add `/metrics` to the *public* routes
   (like `/health`) so standard Prometheus scrapers work without auth. Labels by **connection + run-state** by
   default (no tenant leak); a config flag (`WEIR_METRICS_TENANT_LABELS`) turns on per-tenant labels for
   trusted/internal setups. `metrics` + `metrics-exporter-prometheus`.
2. **Lineage = emit OpenLineage.** Rather than a bespoke internal store, weir emits **OpenLineage `RunEvent`s**
   (START / COMPLETE / FAIL) per run — `job` = the connection, `inputs`/`outputs` = the source/dest datasets,
   facets = row count + timing (+ schema where known). Configurable transport (HTTP endpoint → Marquez/DataHub,
   or console/disabled). This makes weir lineage interoperable with the open ecosystem (ASF-aligned). A light
   UI run-detail lineage view can render the same source→stream→mapping→dest chain from `work_units`.
3. **Scope = all four pillars** (tracing+OTLP, Prometheus, OpenLineage, UI) + decide [[WEIR-A-0022]].

## Proposed decomposition (for sign-off)

- **[[WEIR-A-0022]]** (decide): observability + lineage standard — JSON tracing + OTLP (feature-gated),
  public/aggregate Prometheus, **OpenLineage** for lineage.
- **T-a (tracing):** `weir-cli` JSON tracing + env filter; a `telemetry` cargo feature (OTLP export via
  `opentelemetry-otlp`); spans across request→orchestrator→engine carrying connection/run/tenant.
- **T-b (metrics):** `metrics` + `metrics-exporter-prometheus`; **public `/metrics`**; instrument the run path
  (rows, run-state count, duration histogram, dead-letters, queue depth); aggregate labels + opt-in tenant labels.
- **T-c (OpenLineage):** emit `RunEvent`s (START/COMPLETE/FAIL) with source/dest datasets + row/timing facets;
  pluggable transport (http/console/off); wired into the run lifecycle (engine/orchestrator).
- **T-d (UI lineage panel):** a run-detail lineage view (source→stream→mapping→dest + rows/timing) from
  `work_units`; Aurora graph/list.

## Exit Criteria (draft — refine in design)

- [ ] Structured JSON tracing with connection/run/tenant context; optional OTLP export behind a feature.
- [ ] `/metrics` (Prometheus) exposes rows/run-state/duration/dead-letters/queue-depth, labelled sensibly.
- [ ] A run-lineage view (API + UI panel) shows source→stream→mapping→dest + rows/timing.
- [ ] [[WEIR-A-0022]] decided; workspace + clippy green; the default build stays light (OTel behind a feature).
