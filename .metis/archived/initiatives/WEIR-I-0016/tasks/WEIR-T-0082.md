---
id: cutover-remove-dioxus-re-green
level: task
title: "Cutover — remove Dioxus, re-green Playwright e2e + screenshot, verify pipeline"
short_code: "WEIR-T-0082"
created_at: 2026-07-05T15:36:58.314478+00:00
updated_at: 2026-07-05T16:12:26.781619+00:00
parent: WEIR-I-0016
blocked_by: [WEIR-T-0080, WEIR-T-0081]
archived: true

tags:
  - "#task"
  - "#phase/completed"


exit_criteria_met: false
initiative_id: WEIR-I-0016
---

# Cutover — remove Dioxus, re-green Playwright e2e + screenshot, verify pipeline

## Parent Initiative

[[WEIR-I-0016]] — the closing slice. The Leptos UI reaches parity and Dioxus leaves.

## Objective

Finish the migration: complete any remaining state/API plumbing across the two views, **remove all
Dioxus**, and verify the whole pipeline end-to-end — build, embed, serve, and the Playwright e2e.

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [ ] **No Dioxus anywhere** — `weir-ui/Cargo.toml` has no `dioxus`; `src/main.rs` is fully Leptos; no
  `rsx!`/`use_signal`/`Element` left. `cargo tree -p weir-ui` shows leptos + colliery-io-aurora, not dioxus.
- [ ] **Pipeline green** — `angreal ui build` (trunk) → `dist`; `cargo build -p weir-cli` embeds it;
  `angreal ui demo` comes up and serves the Leptos UI; the earlier Aurora CSS-token `<style>` re-skin is
  gone (Aurora owns styling via `<AuroraStyles/>`).
- [ ] **Playwright e2e passes against the new UI** — `e2e/tests/reverse-etl.spec.ts` (discover →
  onboard `hubspot-dest` → appears as a destination) passes; update selectors for the Aurora DOM if
  needed (Aurora renders different class names than the old hand-rolled CSS). Capture fresh screenshots
  (`screenshot.spec.ts`) for review.
- [ ] **Regression** — the reverse-ETL server env (staged `rest-dest` + `WEIR_DEST_MANIFESTS_DIR`) still
  drives correctly; `curl /catalog/available` lists dest manifests.
- [ ] Workspace + clippy green; no attribution trailers.

## Technical Notes

- The Playwright selectors in `e2e/tests/*.spec.ts` target the **old** DOM (`label.fld`, `select.input`,
  button text "Onboard"). Aurora components emit `cl-*` classes + their own structure — **re-derive the
  locators** against the running Aurora UI (prefer role/text-based locators; read the rendered DOM).
- Run the UI e2e the documented way (`e2e/README.md`): `angreal ui build` + stage connectors + start the
  server with the reverse-ETL env, then `npx playwright test`.
- Delete dead code (the old inline `<style>` is already gone after [[WEIR-T-0079]]; ensure no orphaned
  Dioxus helpers remain).

## Dependencies

- **Blocked by [[WEIR-T-0080]] + [[WEIR-T-0081]]** (both views must exist to reach parity + pass e2e).

## Status Updates

### 2026-07-05 — Cutover complete; Dioxus gone, full suite green

- **No Dioxus** — `cargo tree --manifest-path weir-ui/Cargo.toml` shows `leptos 0.8.20` +
  `colliery-io-aurora 0.1.0`, no dioxus. `main.rs` is fully Leptos (no `rsx!`/`use_signal`/`Element`).
  Renamed the last Dioxus mentions (comments/descriptions) → Leptos across `.gitignore`, `Dockerfile`,
  `weir-api/{build.rs,src/lib.rs,Cargo.toml}`, `.angreal/task_{ui,docker}.py`.
- **Pipeline green** — `angreal ui build` (trunk) → `dist`; `cargo build -p weir-cli` embeds it; server
  serves the Leptos UI. The old inline `<style>` re-skin is gone (Aurora owns styling via `<AuroraStyles/>`).
- **Playwright** — full suite **4/4**: `shell`, `operations` (card → run-detail modal), `reverse-etl`
  (discover → onboard `hubspot-dest` → dest dropdown; selector `label.fld` → `label.weir-fld`), `screenshot`.
- **Regression** — reverse-ETL env drives correctly; `/catalog/available` lists dest manifests
  (`hubspot-dest`, `salesforce-dest`).
- **Clippy** — workspace + `weir-ui` (wasm) clean; no attribution trailers.

**Complete.** [[WEIR-I-0016]] done — weir-ui is a Leptos + Colliery Aurora Dark control plane.
