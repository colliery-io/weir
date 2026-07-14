---
id: setup-view-in-leptos-discover
level: task
title: "Setup view in Leptos — discover/onboard + schema-driven connection form"
short_code: "WEIR-T-0081"
created_at: 2026-07-05T15:36:22.728019+00:00
updated_at: 2026-07-05T16:09:41.956426+00:00
parent: WEIR-I-0016
blocked_by: [WEIR-T-0079]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0016
---

# Setup view in Leptos — discover/onboard + schema-driven connection form

## Parent Initiative

[[WEIR-I-0016]]. Fills the Setup tab of the [[WEIR-T-0079]] shell. This is the view the Playwright
dest-onboarding e2e ([[WEIR-T-0077]]) exercises — so it must preserve those affordances.

## Objective

Reproduce the **Setup view** from the current Dioxus UI in Leptos + Aurora: discover/onboard, paste-a-
manifest with preview, and the schema-driven connection form — feature parity.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **Add a connector** — a picker over `/catalog/available`: source `manifest`s, **`dest-manifest`s
  labelled as destinations**, and crate packages, dropping already-onboarded ones; an **Onboard** button
  posting `/catalog/import` with the right body per kind (`manifest_name` / `dest_manifest_name` / `package`).
  (Preserve the exact behavior the Playwright test asserts.)
- [ ] **Bring your own** — a textarea to paste a declarative manifest (→ `/catalog/preview` shows
  tier/confidence/gaps via `PreviewReport`, then `/catalog/import`) + a crate-path input.
- [ ] **Connection form** — name, **source** + **destination** selects (from `/catalog`, role-filtered;
  dest includes registered dest-manifests), stream select (`/connectors/{p}/discover`), **spec-driven
  fields** (`/connectors/{p}/spec` → `Prop` inputs, written into the config JSON via the `cfg_get`/`cfg_set`
  helpers), raw config JSON textarea, every-secs, and **Save** (`POST /connections`).
- [ ] Aurora inputs (`TextInput`/`Select`/`Textarea` or the crate's equivalents — check `components.rs`),
  `Panel` sections, `Button`s. Toasts on onboard/save/error.
- [ ] `angreal ui build` green; clippy clean.

## Reference

- Port from `weir-ui/src/main.rs` (Setup rsx: the connector picker + onboard body branch, the
  paste/preview block, the connection form + schema fields + `cfg_get`/`cfg_set`). Keep the
  `dest-manifest` grouping/label + the `{ dest_manifest_name }` onboard body verbatim in behavior.
- Aurora inputs/props: `components.rs` is the API truth; the gallery shows working input binding
  (two-way `RwSignal`).

## Dependencies

- **Blocked by [[WEIR-T-0079]]**. Re-greens the Playwright dest-onboarding spec ([[WEIR-T-0077]]).

## Status Updates

### 2026-07-05 — Setup view built (discover/onboard + BYO + connection form)

Ported the full Setup view to Leptos + Aurora (three `Panel`s):
- **Add a connector** — a `<select class="cl-input cl-select">` (Aurora-styled) over `/catalog/available`
  with onboarded dropped (`is_onboarded`), source manifests → dest-manifests → crates; `Onboard` `Button`
  posts `/catalog/import` with `manifest_name` / `dest_manifest_name` / `package` by kind.
- **Bring your own** — paste-manifest textarea + crate-path input; `Preview` (`/catalog/preview` →
  `PreviewReport`) + `Onboard` (`/catalog/import` `{manifest}`/`{path}`).
- **New / edit connection** — name, role-filtered source/dest `<select>`s (from `/catalog`), stream
  select/input (from `/connectors/{src}/discover`), every-secs, **schema-driven fields**
  (`/connectors/{src}/spec` → `Prop`s via `cfg_get`/`cfg_set` into the shared config JSON), raw config JSON,
  `Save` (`POST /connections`). Toasts on all actions.

**Design:** raw `<select>/<input>/<textarea class="cl-input …">` (Aurora-styled via injected CSS) where
value ≠ display (picker: raw name vs friendly label; source/dest: name vs "name · version") — Aurora's
`Select` forces value == label. `Effect` on `src` refetches props + streams; `reload_catalog` after onboard.
Action closures are `Copy` (capture only Copy signals/closures) so they compose into `Callback`s.

**Next:** rebuild (fixing a stream-option move) → seed + Playwright (re-green the dest-onboarding spec) → commit.
