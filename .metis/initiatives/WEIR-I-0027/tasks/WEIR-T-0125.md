---
id: nav-reorg-first-sync-tutorial
level: task
title: "Nav reorg + first-sync tutorial"
short_code: "WEIR-T-0125"
created_at: 2026-07-08T02:10:50.397524+00:00
updated_at: 2026-07-08T02:27:56.786632+00:00
parent: WEIR-I-0027
blocked_by: []
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0027
---

# Nav reorg + first-sync tutorial

## Parent Initiative

[[WEIR-I-0027]] — the structure everything else slots into.

## Objective

Restructure the docs into the four **Diátaxis** modes and write the one **tutorial**: a guided *Your first sync*
that takes a newcomer from install to seeing records land.

## Reference

- `mkdocs.yml` — the nav to rewrite (Home / Getting Started / Guides / API Reference → the four modes).
- `docs/index.md` (rewrite — "early-stage Rust project" is wrong), `docs/getting-started/*` (fold in).
- The e2e/soak local-server flow (`weir init` → `auth token create` → `weir api`; `/catalog/import` →
  `/connections`) as the true, tested happy path for the tutorial.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] `mkdocs.yml` nav = **Tutorials / How-to guides / Reference / Explanation**; the three guides sit under
  How-to; install moved to Reference.
- [x] `docs/index.md` rewritten (OSS ingestion + reverse-ETL, WASM connectors, control plane) with a signpost to
  each mode.
- [x] `docs/tutorials/first-sync.md`: a single **verified** path — install + stage connectors → integration
  Postgres → `weir init` → `connection add slow→postgres` → `run` → `SELECT` the rows. Every command run here.
- [x] `mkdocs build --strict` clean; `getting-started/` folded (installation → Reference, quickstart → tutorial),
  nothing orphaned.

## Status Updates

### 2026-07-08 — done (`29f94a4`)

Four-mode nav; index rewritten; the `first-sync` tutorial is the exact happy path I ran (`slow → postgres`,
5 rows land in `demo_rows`, confirmed by `psql`). Installation expanded (the connector-staging step). `mkdocs
build --strict` clean (ran via `uvx mkdocs-material`). **Complete** — unblocks the How-to/Reference/Explanation
tasks, which add their own nav sections.
