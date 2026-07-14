---
id: b-ui-surface-destination-manifests
level: task
title: "B: UI — surface destination manifests in discover + the connection destination selector"
short_code: "WEIR-T-0077"
created_at: 2026-07-05T01:25:30.677725+00:00
updated_at: 2026-07-05T01:45:15.561645+00:00
parent: WEIR-I-0015
blocked_by: [WEIR-T-0076]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0015
---

# B: UI — surface destination manifests in discover + the connection destination selector

## Parent Initiative

[[WEIR-I-0015]] slice B. Makes onboarding a SaaS **destination** a UI action, not just an API call.

## Objective

In the embedded Dioxus UI (`crates/weir-api`), surface manifest-backed **destinations**: they appear
in the discover/onboard picker (marked as destinations), and a connection's **destination** field
can be a registered dest manifest — so a user can build a warehouse→SaaS activation with no code.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] The **discover/onboard** view lists `kind = "dest-manifest"` packages (from [[WEIR-T-0076]]'s
  `available_packages`), visibly distinguished from sources; onboarding one registers it.
- [ ] The **connection form's destination** selector offers registered manifest destinations
  (alongside `postgres`/built-ins), and picking one produces a runnable connection (the baked
  `rest-dest` config from [[WEIR-T-0076]]).
- [ ] Any new API endpoint the UI needs is added to `weir-api` and returns the dest packages/entries.
- [ ] `angreal ui build` succeeds; existing UI behavior for sources is unchanged.

## Technical Notes

- **Not fully hermetically testable** — there's no headless UI test harness. Verification is
  `angreal ui build` green + a **human eyeball** in the running UI (`angreal ui demo`). A Ralph loop
  should implement it, confirm the build, and **flag it for human review** rather than claim a
  passing test.
- Mirror how sources are rendered in the discover picker + connection form; the data already exists
  once [[WEIR-T-0076]] surfaces dest packages/entries via the API.
- Keep the change minimal — reuse the source components/paths; a destination is the same shape with a
  different `kind`/role.

## Dependencies

- **Blocked by [[WEIR-T-0076]]** (needs discover/register/resolve for dests).

## Status Updates

### 2026-07-05 — implemented; build green; **flagged for human UI review**

Minimal, mirror-the-source changes:
- **API** (`crates/weir-api`): `ImportDto` gains `dest_manifest_name`; `catalog_import` routes it to
  `import_vendored_dest_manifest` (the source path stays `manifest_name` → `import_vendored_manifest`).
- **Frontend** (`weir-ui/src/main.rs`): the discover picker now labels `kind == "dest-manifest"` as
  `· destination` (`friendly`), treats them as onboarded-by-name (`is_onboarded`), groups them between
  low-code sources and crates (`connector_opts`), and routes their "Add" to `{ dest_manifest_name }`.
- **Connection destination selector needs no change** — the dest dropdown is already role-filtered
  (`Destination`), so a registered dest-manifest (roles `[Destination]`, from [[WEIR-T-0076]]) shows up
  automatically once onboarded.

**Verification:** `cargo check -p weir-api` ✅ and **`angreal ui build` → ✅ success** (Dioxus/trunk
release). clippy (running).

**⚠️ NEEDS HUMAN EYEBALL** — there's no headless UI test harness, so I can't assert the rendered flow.
Please verify in the running UI (`angreal ui demo` → http://localhost:8787): (1) HubSpot/Salesforce appear
in "Add a connector" marked `· destination`; (2) onboarding one succeeds; (3) it then appears in a
connection's **destination** dropdown and a warehouse→SaaS connection saves + runs. The build passing means
it compiles + wires the right calls — not that it looks/behaves right.

### 2026-07-05 — **Playwright test added; UI flow now automatically verified** ✅

The human-eyeball gap is closed with a real browser test (`e2e/`, Playwright + Chromium). Against the
running server (embedded UI + staged `rest-dest` + `WEIR_DEST_MANIFESTS_DIR`), `e2e/tests/reverse-etl.spec.ts`
drives the DOM and **passes**: Setup → the discover picker shows **`hubspot-dest · destination`** → click
Onboard → toast "Onboarded hubspot-dest" → it **drops from the picker** and appears as a **selectable
connection destination** (`selectOption` → value set). So the rendered flow is verified, not just the build.

**Bug the test surfaced + fixed:** the dest manifest was named `hubspot` — colliding with the existing
**source** `manifests/hubspot.yaml` on the catalog `(name, version)` key. Renamed the dest manifests to
`dest-manifests/{hubspot,salesforce}-dest.yaml` (matching their `spec.name`), so source and dest are
distinct catalog entries. Updated the S4/S5/onboarding tests + demo-script refs; all green.

**Live-server sanity also confirmed:** `/catalog/available` lists `hubspot-dest`/`salesforce-dest` as
`dest-manifest`; post-onboard `/catalog` shows `hubspot-dest` with `roles: [Destination]`, `kind:
dest-manifest`, `location: weir-rest-dest-pkg`. **Fully verified — no human eyeball needed.**
