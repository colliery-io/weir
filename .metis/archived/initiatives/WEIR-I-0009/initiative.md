---
id: mvp-hardening-configurable
level: initiative
title: "MVP hardening: configurable connections + legible runs"
short_code: "WEIR-I-0009"
created_at: 2026-06-22T12:53:15.992898+00:00
updated_at: 2026-06-22T17:42:07.575070+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: mvp-hardening-configurable
---

# MVP hardening: configurable connections + legible runs Initiative

*Sequenced **before** the parity arc ([[WEIR-I-0008]]): turn the working-but-thin MVP into a usable, legible product. Reviewed via the `angreal ui demo` harness (Echo/Slow/Faulty → ArrowSink covering success/running/dead-letters/failed).*

## Context **[REQUIRED]**

The MVP control loop works and is tested (CLI pipeline, API CRUD/run/history, embedded Dioxus UI). But an MVP review surfaced two load-bearing gaps that make it a demo, not a product — and both gate the parity arc paying off (parity yields many connectors that need **config** and fail in ways you must **debug**):

1. **Config IN — you can't configure a real connection from the UI.** The API DTO carries `config` (JSON), `every_secs`, `cron`, and the binary lists multiple connectors, but the create form sends only `{name, source, dest, stream}` (free-text plugin names). No base_url/credentials/DB-URL, no schedule, no edit/delete, no connector/stream discovery → only the built-in demo defaults are reachable.
2. **Legibility OUT — the UI is a status light, not a story.** weir *persists* most of "what happened" but never surfaces it; logs are dropped entirely:
   - failure reason → `work_units.error` (stored by `mark_failed`) — **not exposed**;
   - dead-letter detail (record + reason) → `dead_letters` table — **only counted**;
   - progress mid-run → derivable from `outbox` (chunks/checkpoints committed) — **not exposed**;
   - resume point → `stream_state.cursor` — **not exposed**;
   - logs/diagnostics → `ReadMessage::Log(_entry) => {}` in the engine — **dropped on the floor**.

This is the MVP slice of Observability & Lineage ([[WEIR-S-0011]] / [[WEIR-A-0022]] draft) — mostly **surfacing state weir already has**, plus capturing logs. Vertical slices: engine/orchestrator → API → UI.

## Goals & Non-Goals **[REQUIRED]**

**Goals:**
- A connection can be **fully configured from the UI**: config (JSON; schema-aware where `spec().config_schema` allows), schedule (`every_secs`/`cron`), edit + delete, and connector/stream **discovery** (no free-text plugin names).
- A run is **legible**: failure reason, dead-letter detail, captured logs/diagnostics, resume cursor, and live progress — surfaced through the API and a UI **run-detail view**.
- The `angreal ui demo` harness stays the living dev surface; each slice is visible there.

**Non-Goals:**
- Full Observability & Lineage / OpenLineage emission ([[WEIR-A-0022]]) — this is the MVP slice; lineage emission is later.
- A dedicated **secrets manager** ([[WEIR-S-0010]] / [[WEIR-A-0021]]) — config may hold secrets inline for now (single-node, open); the secrets backend is a separate initiative. *(Flag inline-secret risk in the UI.)*
- A connector **catalog/registry** ([[WEIR-S-0007]]) — discovery here means "list the linked/registered plugins + `discover()` their streams," not a remote registry.
- Auth/multi-tenant (vendor periphery); connector-builder UI ([[WEIR-A-0017]]).

## Detailed Design **[REQUIRED]**

### Pillar B — Legibility OUT (sharper complaint; mostly surfacing)
**S1 — Expose persisted run state.** Add `error` to the run DTOs (`/runs` + `/connections/{name}/runs`); a dead-letters list endpoint (`record` + `reason`, paginated); expose `stream_state.cursor` (resume point) + `outbox` chunk count (progress). UI: show failure reason on failed feed rows/cards; seed of run-detail.
**S2 — Capture logs/diagnostics.** Engine persists `ReadMessage::Log` + outcome `diagnostics` to a `run_logs` table (today both are discarded); API exposes per-run logs; UI shows them in run-detail.
**S3 — Live progress.** Surface mid-run progress (rows/chunks committed so far, via `outbox` + the lease heartbeat [[WEIR-T-0018]]); UI shows a progress affordance on in-flight runs instead of a bare "running…".
**S6 — Run-detail view (UI).** Consolidated "what happened" page: state/attempt timeline, error, dead-letter list, logs, resume cursor — pulling S1–S3.

### Pillar A — Config IN
**S4 — Configurable + editable connections.** Create/edit form gains a **config** field (raw JSON v0; schema-driven from `spec().config_schema` if present) + schedule (`every_secs`/`cron`); UI **edit + delete** (API already supports delete). Error surfacing on create/run (today `let _ = …` swallows failures).
**S5 — Connector + stream discovery.** API endpoint listing available connectors (from the registered/linked plugins) + `discover()` their streams; UI **dropdowns** replace free-text source/dest/stream — the no-code on-ramp.

## Alternatives Considered **[REQUIRED]**

- **Do parity first, harden later.** Rejected ([[WEIR-I-0008]] note) — parity produces connectors the UI can't configure or debug; legibility + config are prerequisites to the "credible product" bar.
- **Full observability/OpenLineage now.** Deferred — overkill for the MVP; this surfaces existing state + captures logs, leaving lineage emission to [[WEIR-A-0022]]'s own initiative.
- **Schema-driven config only (no raw-JSON fallback).** Rejected for v0 — raw JSON unblocks immediately; schema-driven is layered on where `config_schema` exists.

## Implementation Plan **[REQUIRED]**

Legibility first (the sharper gap, mostly surfacing): **S1 → S2 → S3 → S6**, then config: **S4 → S5**. Each slice lands green + is demoed live via `angreal ui demo`. Decompose into tasks after review.

## Exit Criteria

- [ ] Run failure reason, dead-letter detail, captured logs, resume cursor, and live progress are exposed via the API and shown in a UI run-detail view.
- [ ] A connection can be created/edited/deleted from the UI with config + schedule; source/dest/stream chosen via discovery, not free text.
- [ ] Create/run errors are surfaced (no silent failures).
- [ ] `angreal ui demo` shows all of the above live; workspace + integration suites green; clippy clean.
- [ ] Then start [[WEIR-I-0008]] (Airbyte declarative parity).
