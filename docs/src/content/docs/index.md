---
title: "weir"
---

weir is an **open-source data-movement platform** — ingestion **and** reverse-ETL. It pulls records from
sources (databases, REST APIs, object stores) and lands them in destinations, on a schedule or continuously,
with change-data-capture, typed schemas, and per-tenant isolation.

Its defining choice: **connectors are WASM components**. Each runs sandboxed — no filesystem, network egress only
where the host grants it, and **secrets never enter the connector** (the host injects them on the way out). A
connector is either a small **declarative manifest** (runs on a shared runtime) or a **full-code crate** compiled
to `wasm32-wasip2`. One control-plane binary schedules the work, runs the connectors, exposes an HTTP API, and
serves an embedded web UI.

## Find your way

This documentation follows [Diátaxis](https://diataxis.fr) — four kinds of material for four different needs:

- **[Tutorials](tutorials/first-sync/)** — learning by doing. Start with [Your first sync](tutorials/first-sync/).
- **How-to guides** — steps for a specific task: [map fields](how-to/field-mapping/),
  [author a connector](how-to/connector-authoring/), [soak-test](how-to/soak-testing/), and more.
- **Reference** — precise lookups: [installation](reference/installation/), the [HTTP API](reference/http-api/).
- **Explanation** — the *why*: architecture, the connector/WASM model, tenancy, and delivery guarantees.

New here? **[Install weir](reference/installation/)**, then run **[your first sync](tutorials/first-sync/)**.
