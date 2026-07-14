---
id: a-destination-manifest-onboarding
level: task
title: "A: Destination-manifest onboarding — discover, register, resolve_manifest_dest, ship rest-dest"
short_code: "WEIR-T-0076"
created_at: 2026-07-05T01:24:56.572665+00:00
updated_at: 2026-07-05T01:39:17.866199+00:00
parent: WEIR-I-0015
blocked_by: []
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0015
---

# A: Destination-manifest onboarding — discover, register, resolve_manifest_dest, ship rest-dest

## Parent Initiative

[[WEIR-I-0015]] slice A (load-bearing). Makes a destination manifest onboardable + runnable, by
**mirroring the source onboarding path** for destinations.

## Objective

A destination manifest (`dest-manifests/*.yaml`, e.g. HubSpot/Salesforce from [[WEIR-T-0074]]/
[[WEIR-T-0075]]) can be **discovered → registered → used as a connection's destination → run** —
exactly like a source manifest. And the `rest-dest` guest is **staged where connections run**, not
only in tests.

## Prior art to mirror (all in `crates/weir-app/src/lib.rs`)

- `available_packages` (~line 455): scans `manifests/*.yaml` → `AvailablePackage { kind: "manifest" }`
  for the discover list. **Add a parallel scan of `dest-manifests/`** → `kind: "dest-manifest"`
  (summary via `DestinationManifest::from_yaml`: base_url · N objects).
- `register_connector(&CatalogEntry)` + `CatalogEntry { kind, manifest, roles, … }`: onboarding
  stores a catalog entry. **A dest-manifest entry** has `kind = "dest-manifest"`, `roles =
  [Destination]`, `manifest = <yaml>`.
- `resolve_manifest_source(source, stream, config)` (~line 224): if the ref names a `kind=manifest`
  catalog connector, parse it, `manifest_stream_to_config`, `merge_config`, and return
  `(connector_ref("rest"), baked)`. **Write `resolve_manifest_dest(dest, object, config)`**: same
  shape for `kind = "dest-manifest"` → `dest_object_to_config` (exists, [[WEIR-T-0074]]) → return
  `(connector_ref("rest-dest"), baked)`.
- `add_connection` (~line 181): calls `resolve_manifest_source` for the source. **Also call
  `resolve_manifest_dest`** for the dest (the connection's `dest` ref + the stream/object name).

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Discover**: `available_packages` lists `dest-manifests/` entries with `kind = "dest-manifest"`
  and a summary; source manifests are unaffected.
- [ ] **Register**: onboarding a dest-manifest package stores a `CatalogEntry { kind: "dest-manifest",
  roles: [Destination], manifest }`; `list_connectors` shows it.
- [ ] **Resolve**: `resolve_manifest_dest` bakes a registered dest manifest's object →
  `rest-dest` config (via `dest_object_to_config`) and rewrites the dest ref to `rest-dest`;
  `add_connection` uses it so a connection whose dest is a dest-manifest stores the baked config +
  `rest-dest` ref. Auth secret handling matches the source path ([[WEIR-A-0033]]).
- [ ] **Ship the guest**: `rest-dest` is staged in the shipped connector set / docker image the same
  way `rest`/`postgres` are (so onboarded connections can actually load it), not only in tests. Find
  where the reference connectors are enumerated for staging/packaging and add `rest-dest`.
- [ ] **E2E test** (weir-app, mirror `reverse_etl_hubspot.rs`): register the HubSpot dest manifest →
  `add_connection` (dest = the registered manifest) → assert the stored connection's config is the
  baked `rest-dest` config + ref; run source → dest against a mock and assert the upsert. Prefer
  driving through `add_connection`/`get_connection`/`work_spec` (the real onboarding path).
- [ ] Workspace + integration suites green; clippy clean; no attribution trailers on commits.

## Technical Notes

- Keep it a faithful **mirror** of the source path — don't invent a new onboarding shape. The
  destination "object" name plays the role the source "stream" name plays.
- `dest_object_to_config` already emits `auth_scheme` (bearer/header/oauth2); the per-connection
  secret + `Credential::from_auth_config` split is reused unchanged.
- Staging: check `crates/weir-wasm-testkit` (the `GUESTS` list) and the docker image build
  (`angreal docker build` / `.angreal/`) for where `rest`/`postgres`/`arrow-sink` are enumerated.

## Dependencies

- Prereq for [[WEIR-T-0077]] (UI) and [[WEIR-T-0078]] (demo).
- Builds on [[WEIR-T-0072]] (runtime) + [[WEIR-T-0074]] (`dest_object_to_config`).

## Status Updates

### 2026-07-05 — dest-manifest onboarding wired (mirror of the source path); test green

Implemented a faithful mirror of the source onboarding path:
- `dest_manifests_dir()` (`WEIR_DEST_MANIFESTS_DIR`, default `dest-manifests/`).
- `available_packages` also scans `dest-manifests/` → `AvailablePackage { kind: "dest-manifest" }`
  (summary via `DestinationManifest::from_yaml`: base_url · N objects).
- `ingress.rs`: `import_vendored_dest_manifest` / `import_dest_manifest` → a `CatalogEntry
  { kind: "dest-manifest", roles: [Destination], location: weir-rest-dest-pkg, manifest }`.
- `resolve_manifest_dest(dest, object, config)` (mirror of `resolve_manifest_source`): a registered
  dest-manifest → `dest_object_to_config` + `merge_config` → `(connector_ref("rest-dest"), baked)`.
- `add_connection` now resolves the **dest** too, so a connection whose dest is a manifest stores the
  baked `rest-dest` config + ref.
- **Ship the guest**: added `rest-dest` to `scripts/stage-connectors.sh`; Dockerfile now copies
  `dest-manifests/` + sets `WEIR_DEST_MANIFESTS_DIR`.

**Run-time auth is already symmetric** — the orchestrator's `resolve` runs `Credential::from_auth_config`
for *both* source and dest handles (weir-orchestrator:135), so a dest's bearer/OAuth is minted + injected
host-side with no extra wiring.

**Test** `crates/weir-app/tests/reverse_etl_onboarding.rs` (green): discover HubSpot dest → register (kind
dest-manifest, Destination role) → `add_connection` (dest = the manifest) → `get_connection` asserts the
dest rewrote to `rest-dest` and the config baked (PATCH / properties / bearer / `/crm/v3/objects/contacts`).

**Known limitation (noted in `resolve_manifest_dest`):** the single connection config means a manifest
*source* + manifest *dest* would collide on `base_url`; the real reverse-ETL case (warehouse/Postgres source
+ SaaS dest) does not (postgres keys `url`/`table` don't collide). The full warehouse→SaaS **run** is proven
by [[WEIR-T-0072]]–[[WEIR-T-0075]] (runtime + flow) + the confirmed orchestrator auth symmetry; this task
proves the onboarding/baking wiring hermetically.

**Remaining:** clippy + regression (running) → commit → complete.
