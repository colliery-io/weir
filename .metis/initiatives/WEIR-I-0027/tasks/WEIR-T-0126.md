---
id: how-to-guides-task-oriented
level: task
title: "How-to guides (task-oriented)"
short_code: "WEIR-T-0126"
created_at: 2026-07-08T02:10:51.397139+00:00
updated_at: 2026-07-08T02:47:43.504515+00:00
parent: WEIR-I-0027
blocked_by: [WEIR-T-0125]
archived: false

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0027
---

# How-to guides (task-oriented)

## Parent Initiative

[[WEIR-I-0027]] — "how do I …" for the capabilities shipped since the stubs.

## Objective

Add task-oriented guides for the major capabilities, each a focused problem → steps → done. Keep the voice of the
three existing guides.

## Reference

- Existing (retain): `guides/field-mapping.md`, `guides/connector-authoring.md`, `guides/soak-testing.md`.
- Sources: OIDC/keys ([[WEIR-I-0017]] auth, `weir auth`), tenancy ([[WEIR-A-0036]]/[[WEIR-I-0018]]), CDC + deletes
  ([[WEIR-I-0026]]), manifest onboarding ([[WEIR-S-0015]]/`/catalog/import`), scheduling (`every_secs`/`cron`),
  k8s ([[WEIR-A-0023]], the helm/deploy path).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] Six how-to pages: onboard a declarative connector, schedule syncs, capture changes + propagate deletes,
  secure the control plane, manage tenants, deploy to Kubernetes — each verified against current code/flags.
- [x] Each task-shaped (goal · steps · "done") and navved under **How-to guides** with the three existing guides.
- [x] `mkdocs build --strict` clean.

## Status Updates

### 2026-07-08 — done (`23011e3`)

Six guides written + navved. **Two accuracy findings, documented honestly rather than faked:**
1. **Manifest onboarding is API-only** — the CLI `connection add` with a manifest source fails
   (`weir-frankfurter-pkg not found`); you onboard via `POST /catalog/import {manifest_name}` first.
2. **CDC / write-modes are engine-level today** — `App::work_spec` hardcodes `sync_mode: FullRefresh` +
   `write_mode: Append`, so a CLI/API connection can't yet select CDC, `Upsert` business keys, or `on_delete`.
   The CDC guide documents the proven engine path + flags the connection-exposure gap. **A real product gap
   worth a future initiative** (surface sync/write mode + delete semantics per connection). Complete.
