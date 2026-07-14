---
id: weir-ui-leptos-aurora-dark
level: initiative
title: "weir-ui → Leptos + Aurora Dark"
short_code: "WEIR-I-0016"
created_at: 2026-07-05T15:34:04.163732+00:00
updated_at: 2026-07-05T16:28:32.439520+00:00
parent: WEIR-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: weir-ui-leptos-aurora-dark
---

# weir-ui → Leptos + Aurora Dark Initiative

## Context

Per [[WEIR-A-0035]], `weir-ui` moves from Dioxus to **Leptos 0.8 (CSR)** built from the
**`colliery-io-aurora`** design system (crates.io; lib `aurora_leptos`) — so the control plane is a
real Colliery Aurora Dark app (components, not just tokens). The crate is published + verified
(resolves + compiles). The trunk → `weir-ui/dist` → `weir-api` embed pipeline is framework-agnostic
and stays.

## Goals & Non-Goals

**Goals**
- Reimplement both views (Operations, Setup) in Leptos with Aurora components — feature parity with
  today's Dioxus UI, no behavior loss.
- Consume `colliery-io-aurora` from crates.io; style via `<AuroraStyles/>`.
- Keep the embedded-UI build/serve pipeline working (`angreal ui build`, `weir-api` embed, Playwright).

**Non-Goals**
- New UI features (this is a re-expression, not a redesign).
- Backend/API changes — the ~13 endpoints are unchanged.
- Porting Aurora to Dioxus, or SSR/hydrate (CSR only).

## Current UI inventory (what to reproduce)

- **Models:** Connection, NewConnection, RunRow, CatalogItem, AvailableItem, PreviewReport, LogRow,
  DeadLetterRow, Prop.
- **API (13):** `/connections` (GET/POST), `/connections/{n}` (DELETE), `/connections/{n}/run`,
  `/connections/{c}/logs`, `/connections/{c}/dead-letters`, `/runs`, `/catalog`, `/catalog/available`,
  `/catalog/import`, `/catalog/preview`, `/connectors/{p}/spec`, `/connectors/{p}/discover`.
- **Operations view:** header (wordmark, Operations/Setup tabs, live stats), connection cards
  (name, source→dest flow, state, run button, metrics), run feed, run-detail overlay (logs + dead-letters).
- **Setup view:** "Add a connector" (discover picker + onboard, incl. `dest-manifest`), "Bring your own"
  (paste manifest + preview / crate path), schema-driven connection form (name/source/dest/stream +
  spec-driven fields + config JSON + every-secs + save).
- **Cross-cutting:** live polling (resources on an interval), toasts, config-JSON get/set helpers.

## Implementation Plan (decomposition)

- **[[WEIR-T-0079]] Pipeline + shell:** migrate `weir-ui` to Leptos 0.8 (csr) + `colliery-io-aurora`;
  `AppShell` + header + Operations/Setup tab switch + `<AuroraStyles/>`; trunk builds `dist`,
  `weir-api` embeds it, server serves it. Prove the whole toolchain end-to-end with a minimal shell.
- **[[WEIR-T-0080]] Operations view:** connection cards, run feed, run-detail modal, live polling —
  Aurora `Panel`/`SimpleGrid`/`Table`/`Pill`/`Modal`/`Button`.
- **[[WEIR-T-0081]] Setup view:** discover/onboard (incl. dest manifests), paste-manifest + preview,
  schema-driven connection form — Aurora inputs + `Panel` + `Button`.
- **[[WEIR-T-0082]] Cutover:** finish API/state plumbing, remove Dioxus deps, `angreal ui build` +
  `angreal ui demo` green, Playwright e2e (dest-onboarding spec + screenshot) pass, commit.

T-0079 unblocks the rest; the Playwright dest-onboarding regression re-greens at T-0081/T-0082.

## Exit Criteria

- [x] `weir-ui` is a Leptos + `colliery-io-aurora` app; Dioxus removed.
- [x] Operations + Setup views have parity with the prior UI (cards, feed, run detail, onboard, form).
- [x] `angreal ui build` green; `weir-api` embeds; server serves it.
- [x] Playwright e2e (dest onboarding) passes against the new UI; screenshot reviewed.
- [x] Workspace + clippy green.

**Delivered (commits `7cc5172`..`af212c6`, pushed):** ADR-0035 + T-0079 (shell) + T-0080 (Operations) +
T-0081 (Setup) + T-0082 (cutover). weir-ui is a Leptos 0.8 (CSR) app on `colliery-io-aurora` — real
Aurora components, no Dioxus. Full Playwright suite 4/4 green.
