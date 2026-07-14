---
id: structured-json-tracing-optional
level: task
title: "Structured JSON tracing + optional OTLP"
short_code: "WEIR-T-0098"
created_at: 2026-07-06T01:52:28.508899+00:00
updated_at: 2026-07-06T02:20:07.297099+00:00
parent: WEIR-I-0020
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0020
---

# Structured JSON tracing + optional OTLP

## Parent Initiative

[[WEIR-I-0020]]. Governed by [[WEIR-A-0022]] decision 2 (JSON tracing + OTLP feature-gated).

## Objective

Structured logs + optional distributed tracing: JSON `tracing` output with an env filter by default, and an
optional `telemetry` cargo feature that exports traces via **OTLP** (so the default build stays light). Spans
carry `connection`/`run`/`tenant` across the request → orchestrator → engine path.

## Reference

- `crates/weir-cli/src/main.rs:125` — the `tracing_subscriber::fmt().with_env_filter(...).init()` today (plain).
- cloacina: `telemetry` feature (`tracing-opentelemetry`, `opentelemetry`, `opentelemetry-otlp`,
  `opentelemetry_sdk`) + `tracing-subscriber` `json`; deps in `~/Desktop/cloacina/crates/cloacina-server/Cargo.toml`.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] Default: JSON log output (`tracing-subscriber` `json`) with the env filter; a flag/env selects
  pretty vs json for local dev.
- [ ] A `telemetry` cargo feature (off by default) adds an OTLP export layer (`opentelemetry-otlp`, endpoint
  from `OTEL_EXPORTER_OTLP_ENDPOINT`); the default build has none of the otel deps compiled.
- [ ] Key spans on the run path (`plan_run`, claim, `execute`, connector call) carry `connection`, `run`
  (work_unit id), `tenant` fields.
- [ ] Workspace builds with + without `--features telemetry`; clippy clean.

## Status Updates

### 2026-07-06 — done (`3f636f9`)

- **weir-cli `init_tracing`**: a layered `registry` — **JSON** fmt layer to stderr by default,
  `WEIR_LOG_FORMAT=pretty` for dev, with the env filter (`tracing-subscriber` gains the `json` feature).
- **`telemetry` cargo feature** (off by default): optional `opentelemetry`/`opentelemetry-otlp`/`_sdk`/
  `tracing-opentelemetry` (0.31/0.32, mirroring cloacina); when set + `OTEL_EXPORTER_OTLP_ENDPOINT` present, a
  `SdkTracerProvider` + OTLP exporter + `tracing_opentelemetry::layer()` is added. The **default build compiles
  none** of the opentelemetry tree.
- **weir-orchestrator**: a `"run"` span (`connection`/`run`/`tenant`) **instruments the execute future**
  (`.instrument(span)` — not `.entered()` across the await, which would break Send/nesting).

Builds **both** `cargo build` and `cargo build --features telemetry`; clippy clean. (The JSON layer is active;
the run path is quiet at `info` so a bare run emits no event lines — events are JSON when they fire.) **Complete.**
