---
id: productionize-reverse-etl
level: initiative
title: "Productionize reverse-ETL — onboarding + UI + demo"
short_code: "WEIR-I-0015"
created_at: 2026-07-05T01:24:27.895811+00:00
updated_at: 2026-07-05T02:19:57.711868+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
initiative_id: productionize-reverse-etl
---

# Productionize reverse-ETL — onboarding + UI + demo Initiative

## Context

[[WEIR-I-0007]] built reverse-ETL end-to-end — the shared declarative destination runtime
(`rest-dest`, [[WEIR-A-0034]]), idempotent upsert, and HubSpot + Salesforce as manifests — but
it's **stranded**: proven in wire tests, not usable in the product. A destination manifest can't
be discovered/onboarded, a connection can't point at one, the UI doesn't surface it, and the
demo doesn't show it. This initiative closes the gap between "passes tests" and "a user can do it
and we can show it."

The source path already does all of this; the work is to **mirror it on the destination side**.

## Goals & Non-Goals

**Goals**
- Onboard a **destination manifest** end-to-end: discover → register → a connection whose dest is
  a manifest-backed SaaS → run, exactly like a source manifest.
- Surface destination manifests in the **UI** (discover/onboard + the connection destination
  selector).
- A **demo beat**: Postgres warehouse → HubSpot, through the real onboarding flow.

**Non-Goals**
- New destination *capabilities* (that's the runtime, [[WEIR-I-0007]] — done). This is plumbing +
  surface.
- More SaaS destinations, Bulk API, live-key CI (those are [[WEIR-I-0014]] / follow-ups).

## Architecture

The source side: `available_packages` scans `manifests/` → discover list; `register_connector`
stores a `kind = "manifest"` catalog entry; `add_connection` → `resolve_manifest_source` bakes the
stream config via `manifest_stream_to_config` and rewrites the source ref to `rest`. **Mirror each
step for destinations** — `dest-manifests/` scan, a `kind = "dest-manifest"` entry,
`resolve_manifest_dest` baking via `dest_object_to_config` (already exists, [[WEIR-T-0074]]) and
rewriting the dest ref to `rest-dest`. The `rest-dest` guest must also be **staged where
connections run** (docker image / connector set), not only in tests.

## Implementation Plan (decomposition)

- **[[WEIR-T-0076]] A — Backend onboarding** (load-bearing): dest-manifest discovery + registration
  + `resolve_manifest_dest` in `add_connection` + stage `rest-dest` in the shipped connector set.
  Onboard HubSpot dest → create a connection → assert the baked `rest-dest` config; run E2E.
- **[[WEIR-T-0077]] B — UI**: dest manifests in the discover/onboard list + the connection
  **destination** selector (embedded Dioxus UI in `weir-api` + API endpoints). *Not fully
  hermetically testable — needs a human eyeball in the running UI.*
- **[[WEIR-T-0078]] C — Demo**: a reverse-ETL beat in the demo script (Postgres warehouse → HubSpot via
  the onboarding flow).

A unblocks B and C. B is the piece a Ralph loop can't self-verify.

## Exit Criteria

- [ ] A destination manifest onboards through the normal path (discover → register → connection →
  run); the `rest-dest` guest ships in the connector set.
- [ ] The UI lets a user pick a manifest-backed SaaS **destination** for a connection.
- [ ] The demo script has a reverse-ETL beat that works against the shipped build.
- [ ] Workspace + integration suites green; clippy clean.
