---
id: phase-1-migratable-core
level: initiative
title: "Phase 1: Migratable Core"
short_code: "WEIR-I-0002"
created_at: 2026-06-17T21:23:59.038389+00:00
updated_at: 2026-06-19T13:33:36.407884+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: XL
initiative_id: phase-1-migratable-core
---

# Phase 1: Migratable Core Initiative

*Phase 1 of the weir vision ([[WEIR-V-0001]]): a credible Airbyte replacement for a real pipeline — the Rust core running the native connector contract end-to-end, single-node deploy, plus declarative-connector migration. This initiative consumes the decided architecture from [[WEIR-I-0001]] (Foundations & Design) and turns it into running software.*

## Context **[REQUIRED]**

Foundations & Design ([[WEIR-I-0001]]) decided the platform spine: language split ([[WEIR-A-0001]]), trust-tiered execution ([[WEIR-A-0002]]: dylib + WASM via `fidius`), durable store ([[WEIR-A-0009]]: `diesel-dual-db`), transactional-outbox work distribution ([[WEIR-A-0010]]), secret handling ([[WEIR-A-0013]]), packaging ([[WEIR-A-0015]]), and the connector contract v0 design ([[WEIR-A-0014]] / [[WEIR-S-0006]]). The one decision still gated is `0014` itself — its agreed gate to `decided` is a **reference connector that round-trips over both execution primitives**. That gate is the natural first slice of implementation.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A **walking skeleton**: one declarative REST source → one native Arrow destination, round-tripping through the real Sync Engine with transactional checkpointing — over **both** execution primitives (dylib + WASM). This **ratifies [[WEIR-A-0014]]**.
- Build out from the skeleton toward the vision's Phase-1 bar: native connector contract, basic scheduler, single-node deploy, and the declarative-connector migration importer for a credible one-pipeline Airbyte replacement.

**Non-Goals:**
- Full connector catalog breadth, reverse-ETL connectors (HubSpot/Salesforce), connector builder UI, OpenLineage — those are Phase 2.
- Multi-node / Kubernetes operator / autoscaler — Phase 3 (periphery).
- Anything in the vendor periphery.

## Detailed Design **[REQUIRED]**

Implementation is **vertical-slice first**: build the thinnest end-to-end path that proves the architecture, then widen. Component start order follows the execution spine (per the component map and the first-wave ADRs).

### Slice 1 — Walking skeleton (contract round-trip) — *decomposed now*
The first vertical slice; its completion ratifies [[WEIR-A-0014]]. Tasks `WEIR-T-0001..0006` (below). Touches: Connector Contract & SDK ([[WEIR-S-0006]]), Runtime ([[WEIR-S-0005]]), Sync Engine ([[WEIR-S-0004]]), Store ([[WEIR-S-0009]]).

### Later slices (roadmap; decomposed when Slice 1 lands)
2. **Scheduling & runs** — cron/manual triggers, backfills, retries ([[WEIR-S-0004]], [[WEIR-A-0011]]). *Decomposed: [[WEIR-T-0007]] run lifecycle/RunManager, [[WEIR-T-0008]] retries+dead-letter, [[WEIR-T-0009]] scheduler, [[WEIR-T-0010]] backfill.*
3. **Control Plane API + config model** — CRUD over the connection-centric model ([[WEIR-S-0002]], [[WEIR-A-0007]], [[WEIR-A-0006]]). *DONE: config model in Slice 4 (`connections`); [[WEIR-T-0016]] axum control-plane API (CRUD + run + history).*
4. **Single-node deploy** — one binary, embedded SQLite, no broker ([[WEIR-S-0012]], NFR-DP-1). *DONE: [[WEIR-T-0014]] `weir` binary + connection config model; [[WEIR-T-0015]] `weir serve` daemon.*
5. **Migration importer (declarative)** — Airbyte YAML → manifest → connector ([[WEIR-S-0008]], [[WEIR-A-0020]]). *DONE: [[WEIR-T-0012]] `weir-importer` (Airbyte low-code → weir manifest) + [[WEIR-T-0013]] fidelity (Airbyte → byte-identical runnable connector).*
6. **Minimal Web UI** — configure + monitor ([[WEIR-S-0003]]). *DONE: [[WEIR-T-0017]] embedded Dioxus (WASM) UI served by `weir api`.*

## Alternatives Considered **[REQUIRED]**

- **Horizontal build (all of one component, then the next).** Rejected: defers integration risk; the architecture's open questions live at the *seams* (executor seam, checkpoint+outbox transaction), which only a vertical slice exercises.
- **Ratify `0014` on paper, build later.** Rejected: we explicitly chose the reference-connector round-trip as the gate ([[WEIR-A-0014]]).

## Implementation Plan **[REQUIRED]**

Slice 1 (now), tasks in rough dependency order:
1. `WEIR-T-0001` — Connector contract crate (interface + boundary types).
2. `WEIR-T-0002` — Executor wiring: host dylib + WASM primitives behind the seam.
3. `WEIR-T-0003` — Reference declarative REST source (manifest v0 → codegen).
4. `WEIR-T-0004` — Reference native Arrow destination (dylib).
5. `WEIR-T-0005` — Minimal Sync Engine slice (plan → outbox dispatch → read→map→write → transactional checkpoint).
6. `WEIR-T-0006` — End-to-end round-trip conformance test over both primitives → **ratify [[WEIR-A-0014]]**.

## Exit Criteria

- [x] Slice 1 walking skeleton green over both execution primitives; `WEIR-A-0014` `decided`.
- [x] Later slices (2–6) decomposed and delivered: scheduling/runs + orchestration (2), control-plane API (3), single-node deploy (4), migration importer (5), web UI (6).
- [x] A real pipeline runs end-to-end on single-node — `weir run`/`weir serve` over the orchestrator + engine; `weir api` exposes it + the embedded Dioxus UI.
