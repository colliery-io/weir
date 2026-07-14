---
id: 001-web-ui-on-leptos-the-aurora-dark
level: adr
title: "Web UI on Leptos + the Aurora Dark design system"
number: 35
short_code: "WEIR-A-0035"
created_at: 2026-07-05T15:32:49.317663+00:00
updated_at: 2026-07-05T15:32:49.317663+00:00
decision_date: 2026-07-05
decision_maker: dylan.storey@gmail.com
parent:
archived: false

tags:
  - "#adr"
  - "#phase/decided"


exit_criteria_met: false
initiative_id: NULL
---

# ADR-35: Web UI on Leptos + the Aurora Dark design system

## Context

The weir control-plane UI (`weir-ui`, an embedded WASM SPA served by `weir-api`) is written in
**Dioxus** with a hand-rolled stylesheet. We want the UI to be a **Colliery Aurora Dark** app —
consistent with the org's other control planes (cloacina et al.) and built from the shared component
library rather than bespoke chrome. The Aurora design system ships as **`aurora-leptos`** (published
on crates.io as **`colliery-io-aurora`**, lib `aurora_leptos`) — a **Leptos** component pack (tokens,
primitives, data-display widgets, DAG). A prior step re-skinned the Dioxus UI with Aurora's CSS
tokens, but the **components** (`AppShell`, `Panel`, `Table`, `Pill`, `Modal`, …) are Leptos-only and
can't be used from Dioxus.

## Decision

**Rewrite `weir-ui` in Leptos 0.8 (CSR) and build it from `colliery-io-aurora`.** weir consumes the
design system as a normal crates.io dependency (`colliery-io-aurora = "0.1"`, imported as
`aurora_leptos`) and renders the two views (Operations, Setup) with Aurora components + `<AuroraStyles/>`
for the stylesheet. The embedded-UI pipeline is unchanged in shape: **trunk** builds the WASM SPA into
`weir-ui/dist`, which `weir-api/build.rs` embeds into the binary.

## Alternatives Analysis

| Option | Pros | Cons | Risk | Cost |
|--------|------|------|------|------|
| **Leptos + `colliery-io-aurora`** (chosen) | Real Aurora components, not just tokens; consistent with Colliery control planes; the pack renders, we supply meaning | Full ~1000-line frontend rewrite; new framework in the tree | Med | High (one-time rewrite) |
| **Stay on Dioxus, keep the CSS-token re-skin** | No rewrite | Only the *palette* is Aurora — none of the components/widgets/DAG; drifts from the org design system; re-implements chrome forever | Low | Low now, high forever |
| **Port Aurora's components to Dioxus** | Keep Dioxus | Fork/maintain a second copy of the design system; defeats "shared library" | High | Very high |

## Rationale

The value of Aurora is the **component library + widgets + DAG**, not just the colors — and those are
Leptos. Consuming the published crate means weir tracks the org design system by version bump instead
of hand-porting chrome. The UI is a self-contained SPA (one crate, ~13 API endpoints, two views), so
the blast radius of a framework switch is contained to `weir-ui` (+ the trunk/embed wiring, which is
framework-agnostic). Publishing `colliery-io-aurora` to crates.io (done) removes the only real
blocker — CI/docker build it from crates.io with no path/git/submodule coupling to `aurora-dark`.

## Consequences

### Positive
- weir looks + behaves like a Colliery Aurora control plane, from the shared library.
- Design-system updates arrive by dependency bump; no bespoke chrome to maintain.
- Access to Aurora's data-display widgets (status pills, meters, banners) + DAG drawing for future views.

### Negative
- A one-time full rewrite of `weir-ui` (Dioxus → Leptos).
- Leptos 0.8 + a crates.io design-system dep enter the build; UI has no headless Rust test (mitigated
  by `angreal ui build` + the Playwright e2e suite).

### Neutral
- The trunk → `weir-ui/dist` → `weir-api` embed pipeline is unchanged.
- The earlier Dioxus CSS-token re-skin is superseded (Aurora ships its own stylesheet via `AuroraStyles`).

## Relationships

- **Supersedes** the Dioxus Aurora CSS-token re-skin (commit `1efc341`).
- **Realized by** [[WEIR-I-0016]] (the weir-ui Leptos migration).
- Depends on `colliery-io-aurora` (crates.io) — the published `aurora-leptos` design pack.
