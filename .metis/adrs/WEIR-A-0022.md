---
id: 001-observability-lineage-standard
level: adr
title: "Observability & lineage standard"
number: 1
short_code: "WEIR-A-0022"
created_at: 2026-06-17T02:12:28.131399+00:00
updated_at: 2026-07-06T01:52:14.368703+00:00
decision_date:
decision_maker:
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-0022: Observability & lineage standard

**Status:** Decided (2026-07-06, dylan.storey) — finalized under [[WEIR-I-0020]]. *Raised by: [[WEIR-S-0011]]
Observability & Lineage, [[WEIR-S-0013]] Integration Adapters.*

## Context **[REQUIRED]**

Lineage is an integration anchor (DataHub) and a differentiator. We must choose a standard and transport so emission is portable, async, and pluggable across sinks.

## Decision **[REQUIRED]**

Three standards, finalized under [[WEIR-I-0020]]:

1. **Lineage = OpenLineage.** Emit OpenLineage `RunEvent`s (START/COMPLETE/FAIL) **asynchronously** (never
   blocking the run path): `job` = the connection, `inputs`/`outputs` = source/dest datasets, facets = row count
   + timing (+ schema where known). **Pluggable transport** — HTTP (Marquez/DataHub), console, or off. Portable,
   vendor-neutral.
2. **Tracing = `tracing` + JSON, OTLP feature-gated.** Structured JSON logs with an env filter by default; an
   optional `telemetry` cargo feature exports traces via **OTLP** (`opentelemetry-otlp`) so the default build
   stays light. Spans carry `connection`/`run`/`tenant`.
3. **Metrics = Prometheus, public + aggregate.** A **public** `/metrics` endpoint (`metrics` +
   `metrics-exporter-prometheus`), labelled by **connection + run-state** (aggregate — no tenant leak to
   unauthenticated scrapers); per-tenant labels are **opt-in** via config (`WEIR_METRICS_TENANT_LABELS`) for
   trusted/internal setups.

## Alternatives Analysis **[CONDITIONAL: Complex Decision]**

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| OpenLineage (chosen) | Open standard; DataHub & others consume it; no lock-in | Schema mapping work | Low | Medium |
| Vendor-specific hooks | Tight DataHub integration | Lock-in; against vendor-neutrality | Medium | Low |

## Rationale **[REQUIRED]**

Riding an open standard keeps the metadata plane vendor-neutral (a vision principle) and makes weir a first-class citizen regardless of catalog vendor.

## Consequences **[REQUIRED]**

### Positive
- Portable lineage; integration anchor for DataHub and beyond.

### Negative
- Must map weir's run model onto OpenLineage entities.

### Neutral
- Consumed by Integration Adapters ([[WEIR-S-0013]]).
